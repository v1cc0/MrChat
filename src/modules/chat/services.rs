use std::sync::Arc;

use anyhow::Result;

use crate::shared::db::TursoPool;

/// Container for chat-related service objects.
#[derive(Clone)]
pub struct ChatServices {
    db: Arc<TursoPool>,
    // llm: TODO add LLM client once interface is defined.
}

impl ChatServices {
    pub fn new(db: Arc<TursoPool>) -> Self {
        Self { db }
    }

    pub fn pool(&self) -> Arc<TursoPool> {
        self.db.clone()
    }

    /// Placeholder for ensuring the pipeline works end-to-end.
    pub async fn ping(&self) -> Result<()> {
        // Run a minimal query to confirm connectivity when wiring up the module.
        let conn = self.db.connection()?;
        conn.query("SELECT 1", ()).await?;
        Ok(())
    }
}
