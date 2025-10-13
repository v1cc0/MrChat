use std::{sync::Arc, time::SystemTime};

use anyhow::{Context, Result, bail};
use gpui::Global;
use isahc::{AsyncReadResponseExt, prelude::*};
use serde_json::json;

use crate::{config::ChatSection, shared::db::TursoPool};

use super::{
    models::{ConversationId, ConversationSummary, Message, MessageRole},
    storage::ChatDao,
};

/// Container for chat-related service objects.
#[derive(Clone)]
pub struct ChatServices {
    db: Arc<TursoPool>,
    dao: ChatDao,
    chat_config: ChatSection,
    api_key: Option<String>,
}

impl ChatServices {
    pub fn new(db: Arc<TursoPool>, chat_config: ChatSection, api_key: Option<String>) -> Self {
        let dao = ChatDao::new(db.clone());
        let api_key = chat_config
            .api_key
            .clone()
            .filter(|k| !k.is_empty())
            .or(api_key.filter(|k| !k.is_empty()));

        Self {
            db,
            dao,
            chat_config,
            api_key,
        }
    }

    pub fn pool(&self) -> Arc<TursoPool> {
        self.db.clone()
    }

    pub fn dao(&self) -> &ChatDao {
        &self.dao
    }

    pub fn chat_config(&self) -> &ChatSection {
        &self.chat_config
    }

    pub async fn ensure_schema(&self) -> Result<()> {
        self.dao.ensure_schema().await
    }

    pub async fn list_conversations(&self) -> Result<Vec<ConversationSummary>> {
        self.dao.list_conversations().await
    }

    pub async fn fetch_conversation(
        &self,
        id: &ConversationId,
    ) -> Result<Option<ConversationSummary>> {
        self.dao.get_conversation(id).await
    }

    pub async fn create_conversation(
        &self,
        title: &str,
        model_id: &str,
    ) -> Result<ConversationSummary> {
        let id = ConversationId::generate();
        self.dao
            .create_conversation(id, title, model_id, SystemTime::now())
            .await
    }

    pub async fn rename_conversation(&self, id: &ConversationId, title: &str) -> Result<()> {
        self.dao
            .update_conversation_title(id, title, SystemTime::now())
            .await
    }

    pub async fn delete_conversation(&self, id: &ConversationId) -> Result<()> {
        self.dao.delete_conversation(id).await
    }

    pub async fn list_messages(&self, id: &ConversationId) -> Result<Vec<Message>> {
        self.dao.list_messages(id).await
    }

    pub async fn append_message(
        &self,
        conversation_id: ConversationId,
        role: MessageRole,
        content: impl Into<String>,
        token_usage: Option<u32>,
    ) -> Result<Message> {
        let mut message = Message::new(conversation_id, role, content.into());
        if token_usage.is_some() {
            message = message.with_token_usage(token_usage);
        }
        self.dao.append_message(&message).await?;
        Ok(message)
    }

    pub async fn store_message(&self, message: Message) -> Result<Message> {
        self.dao.append_message(&message).await?;
        Ok(message)
    }

    pub async fn generate_assistant_reply(
        &self,
        conversation_id: &ConversationId,
    ) -> Result<Option<Message>> {
        if self.chat_config.api_endpoint.is_empty() {
            return Ok(None);
        }

        let history = self.dao.list_messages(conversation_id).await?;

        let mut payload_messages = Vec::new();
        for message in history
            .iter()
            .rev()
            .take(50)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
        {
            let role = match message.role {
                MessageRole::User => "user",
                MessageRole::Assistant => "assistant",
                MessageRole::System => "system",
                MessageRole::Tool => "tool",
            };
            payload_messages.push(json!({
                "role": role,
                "content": message.content,
            }));
        }

        let payload = json!({
            "model": self.chat_config.default_model,
            "messages": payload_messages,
        });

        let mut request = isahc::http::Request::builder()
            .method(isahc::http::Method::POST)
            .uri(&self.chat_config.api_endpoint)
            .header("content-type", "application/json");

        if let Some(key) = self.api_key.as_ref() {
            request = request.header("authorization", format!("Bearer {}", key));
        }

        let request = request
            .body(serde_json::to_vec(&payload)?)
            .context("failed to build assistant request")?;

        let mut response = isahc::send_async(request)
            .await
            .context("failed to send assistant request")?;

        if !response.status().is_success() {
            let body = response.text().await.unwrap_or_default();
            bail!(
                "assistant request failed with status {}: {}",
                response.status(),
                body
            );
        }

        let body = response
            .text()
            .await
            .context("failed to read assistant response body")?;
        let parsed: serde_json::Value =
            serde_json::from_str(&body).context("failed to parse assistant response json")?;

        let reply_text = parsed
            .pointer("/choices/0/message/content")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .or_else(|| {
                parsed
                    .pointer("/choices/0/text")
                    .and_then(|v| v.as_str().map(|s| s.to_string()))
            })
            .context("assistant response missing content")?;

        let message = Message::new(conversation_id.clone(), MessageRole::Assistant, reply_text);
        self.dao.append_message(&message).await?;

        Ok(Some(message))
    }

    /// Placeholder for ensuring the pipeline works end-to-end.
    pub async fn ping(&self) -> Result<()> {
        // Ensure schema exists, then run a minimal query to confirm connectivity.
        self.ensure_schema().await?;
        let conn = self.db.connection()?;
        conn.query("SELECT 1", ()).await?;
        Ok(())
    }
}

impl Global for ChatServices {}
