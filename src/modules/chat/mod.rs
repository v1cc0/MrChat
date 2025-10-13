pub mod models;
pub mod services;
pub mod ui;

use std::sync::Arc;

use anyhow::Result;

use crate::shared::db::TursoPool;

/// High level entry point for chat functionality.
pub struct ChatFacade {
    db: Arc<TursoPool>,
}

impl ChatFacade {
    pub fn new(db: Arc<TursoPool>) -> Self {
        Self { db }
    }

    pub fn db(&self) -> Arc<TursoPool> {
        Arc::clone(&self.db)
    }

    pub fn services(&self) -> services::ChatServices {
        services::ChatServices::new(self.db.clone())
    }
}

/// Bootstrap helper used when we only have configuration yet.
pub async fn init_from_pool(pool: TursoPool) -> Result<ChatFacade> {
    Ok(ChatFacade::new(Arc::new(pool)))
}
