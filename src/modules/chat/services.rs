use std::{sync::Arc, time::SystemTime};

use anyhow::Result;
use gpui::Global;

use crate::shared::db::TursoPool;

use super::{
    models::{ConversationId, ConversationSummary, Message, MessageRole},
    storage::ChatDao,
};

/// Container for chat-related service objects.
#[derive(Clone)]
pub struct ChatServices {
    db: Arc<TursoPool>,
    dao: ChatDao,
    // llm: TODO add LLM client once interface is defined.
}

impl ChatServices {
    pub fn new(db: Arc<TursoPool>) -> Self {
        let dao = ChatDao::new(db.clone());
        Self { db, dao }
    }

    pub fn pool(&self) -> Arc<TursoPool> {
        self.db.clone()
    }

    pub fn dao(&self) -> &ChatDao {
        &self.dao
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
