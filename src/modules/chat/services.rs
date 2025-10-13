use std::sync::Arc;

use anyhow::Result;

use crate::shared::db::TursoPool;

use super::storage::ChatDao;

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

    /// Placeholder for ensuring the pipeline works end-to-end.
    pub async fn ping(&self) -> Result<()> {
        // Ensure schema exists, then run a minimal query to confirm connectivity.
        self.ensure_schema().await?;
        let conn = self.db.connection()?;
        conn.query("SELECT 1", ()).await?;
        Ok(())
    }
}
