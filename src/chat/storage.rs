use std::{
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, anyhow};
use turso::Row;

use crate::shared::db::{TursoConnection, TursoDatabase};

use super::models::{ConversationId, ConversationSummary, Message, MessageRole};

const DDL_CONVERSATIONS: &str = r#"
CREATE TABLE IF NOT EXISTS conversations (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    model_id TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    metadata TEXT
)"#;

const DDL_MESSAGES: &str = r#"
CREATE TABLE IF NOT EXISTS messages (
    id TEXT PRIMARY KEY,
    conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    role TEXT NOT NULL,
    content TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    token_usage INTEGER,
    metadata TEXT
)"#;

/// Data access object for chat domain entities using Turso.
#[derive(Clone)]
pub struct ChatDao {
    pool: Arc<TursoDatabase>,
}

impl ChatDao {
    pub fn new(pool: Arc<TursoDatabase>) -> Self {
        Self { pool }
    }

    fn connection(&self) -> Result<TursoConnection> {
        self.pool.connect()
    }

    /// Ensure the minimal schema required by the chat module exists.
    pub async fn ensure_schema(&self) -> Result<()> {
        let conn = self.connection()?;
        conn.execute(DDL_CONVERSATIONS, ()).await?;
        conn.execute(DDL_MESSAGES, ()).await?;
        Ok(())
    }

    pub async fn list_conversations(&self) -> Result<Vec<ConversationSummary>> {
        let conn = self.connection()?;
        let mut rows = conn
            .query(
                r#"
                SELECT id, title, updated_at, model_id
                FROM conversations
                ORDER BY updated_at DESC
                "#,
                (),
            )
            .await?;

        let mut conversations = Vec::new();
        while let Some(row) = rows.next().await? {
            conversations.push(row_to_conversation_summary(&row)?);
        }

        Ok(conversations)
    }

    pub async fn get_conversation(
        &self,
        id: &ConversationId,
    ) -> Result<Option<ConversationSummary>> {
        let conn = self.connection()?;
        let mut rows = conn
            .query(
                r#"
                SELECT id, title, updated_at, model_id
                FROM conversations
                WHERE id = ?1
                "#,
                [id.0.as_str()],
            )
            .await?;

        if let Some(row) = rows.next().await? {
            Ok(Some(row_to_conversation_summary(&row)?))
        } else {
            Ok(None)
        }
    }

    pub async fn create_conversation(
        &self,
        id: ConversationId,
        title: &str,
        model_id: &str,
        timestamp: SystemTime,
    ) -> Result<ConversationSummary> {
        let conn = self.connection()?;
        let ts = to_millis(timestamp);
        conn.execute(
            r#"
            INSERT INTO conversations (id, title, model_id, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?4)
            "#,
            (id.0.as_str(), title, model_id, ts),
        )
        .await?;

        Ok(ConversationSummary {
            id,
            title: title.to_string(),
            updated_at: timestamp,
            model_id: model_id.to_string(),
        })
    }

    pub async fn update_conversation_title(
        &self,
        id: &ConversationId,
        title: &str,
        timestamp: SystemTime,
    ) -> Result<()> {
        let conn = self.connection()?;
        conn.execute(
            r#"
            UPDATE conversations
            SET title = ?2, updated_at = ?3
            WHERE id = ?1
            "#,
            (id.0.as_str(), title, to_millis(timestamp)),
        )
        .await?;
        Ok(())
    }

    pub async fn delete_conversation(&self, id: &ConversationId) -> Result<()> {
        let conn = self.connection()?;
        conn.execute(
            r#"DELETE FROM conversations WHERE id = ?1"#,
            [id.0.as_str()],
        )
        .await?;
        Ok(())
    }

    pub async fn list_messages(&self, conversation_id: &ConversationId) -> Result<Vec<Message>> {
        let conn = self.connection()?;
        let mut rows = conn
            .query(
                r#"
                SELECT id, conversation_id, role, content, created_at, token_usage
                FROM messages
                WHERE conversation_id = ?1
                ORDER BY created_at ASC
                "#,
                [conversation_id.0.as_str()],
            )
            .await?;

        let mut messages = Vec::new();
        while let Some(row) = rows.next().await? {
            messages.push(row_to_message(&row)?);
        }

        Ok(messages)
    }

    pub async fn append_message(&self, message: &Message) -> Result<()> {
        let conn = self.connection()?;
        conn.execute(
            r#"
            INSERT INTO messages (id, conversation_id, role, content, created_at, token_usage)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
            (
                message.id.as_str(),
                message.conversation_id.0.as_str(),
                role_to_str(&message.role),
                message.content.as_str(),
                to_millis(message.created_at),
                message.token_usage.map(|v| v as i64),
            ),
        )
        .await?;

        // keep conversation updated_at in sync
        conn.execute(
            r#"
            UPDATE conversations
            SET updated_at = ?2
            WHERE id = ?1
            "#,
            (
                message.conversation_id.0.as_str(),
                to_millis(message.created_at),
            ),
        )
        .await?;

        Ok(())
    }
}

fn row_to_conversation_summary(row: &Row) -> Result<ConversationSummary> {
    Ok(ConversationSummary {
        id: ConversationId::new(row.get::<String>(0)?),
        title: row.get::<String>(1)?,
        updated_at: from_millis(row.get::<i64>(2)?)
            .context("invalid updated_at stored for conversation")?,
        model_id: row.get::<String>(3)?,
    })
}

fn row_to_message(row: &Row) -> Result<Message> {
    let role_raw: String = row.get(2)?;
    let role = str_to_role(&role_raw)?;
    Ok(Message {
        id: row.get::<String>(0)?,
        conversation_id: ConversationId::new(row.get::<String>(1)?),
        role,
        content: row.get::<String>(3)?,
        created_at: from_millis(row.get::<i64>(4)?)
            .context("invalid created_at stored for message")?,
        token_usage: row.get::<Option<i64>>(5)?.map(|v| v as u32),
    })
}

fn to_millis(ts: SystemTime) -> i64 {
    ts.duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

fn from_millis(value: i64) -> Result<SystemTime> {
    let millis = u64::try_from(value).map_err(|_| anyhow!("timestamp {value} below epoch"))?;
    Ok(UNIX_EPOCH + Duration::from_millis(millis))
}

fn role_to_str(role: &MessageRole) -> &'static str {
    match role {
        MessageRole::System => "system",
        MessageRole::User => "user",
        MessageRole::Assistant => "assistant",
        MessageRole::Tool => "tool",
    }
}

fn str_to_role(src: &str) -> Result<MessageRole> {
    match src {
        "system" => Ok(MessageRole::System),
        "user" => Ok(MessageRole::User),
        "assistant" => Ok(MessageRole::Assistant),
        "tool" => Ok(MessageRole::Tool),
        other => Err(anyhow!("unknown message role: {other}")),
    }
}
