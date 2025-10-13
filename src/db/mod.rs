use std::path::Path;

use anyhow::{Context, Result};
use std::fs;

use turso::{Connection, Database, params::IntoParams};

pub struct TursoDatabase {
    inner: Database,
}

impl TursoDatabase {
    pub async fn open_local(path: impl AsRef<Path>) -> Result<Self> {
        let db = turso::Builder::new_local(path.as_ref().to_string_lossy().as_ref())
            .build()
            .await
            .context("failed to open local Turso database")?;
        Ok(Self { inner: db })
    }

    pub fn connect(&self) -> Result<TursoConnection> {
        let conn = self
            .inner
            .connect()
            .context("failed to connect to turso database")?;
        Ok(TursoConnection { inner: conn })
    }

    pub async fn run_migrations(&self, migrations_dir: impl AsRef<Path>) -> Result<()> {
        let mut entries: Vec<_> = fs::read_dir(migrations_dir.as_ref())
            .context("unable to read migrations directory")?
            .filter_map(|entry| entry.ok())
            .filter(|entry| {
                entry
                    .path()
                    .extension()
                    .map(|ext| ext == "sql")
                    .unwrap_or(false)
            })
            .collect();

        entries.sort_by_key(|entry| entry.path());

        for entry in entries {
            let path = entry.path();
            let sql = fs::read_to_string(&path)
                .with_context(|| format!("failed to read migration {:?}", path))?;

            let conn = self.connect()?;
            conn.execute_batch(sql.as_str()).await.with_context(|| {
                format!(
                    "failed to execute migration {:?}",
                    path.file_name().unwrap()
                )
            })?;
        }

        Ok(())
    }
}

pub struct TursoConnection {
    inner: Connection,
}

impl TursoConnection {
    pub async fn execute(&self, sql: &str, params: impl IntoParams) -> Result<u64> {
        self.inner
            .execute(sql, params)
            .await
            .context("turso execute failed")
    }

    pub async fn query(&self, sql: &str, params: impl IntoParams) -> Result<turso::Rows> {
        self.inner
            .query(sql, params)
            .await
            .context("turso query failed")
    }

    pub async fn execute_batch(&self, sql: &str) -> Result<()> {
        self.inner
            .execute_batch(sql)
            .await
            .context("turso execute batch failed")
    }

    pub async fn query_map<T, F>(
        &self,
        sql: &str,
        params: impl IntoParams,
        mut f: F,
    ) -> Result<Vec<T>>
    where
        F: FnMut(&turso::Row) -> Result<T>,
    {
        let mut rows = self.query(sql, params).await?;
        let mut buffer = Vec::new();
        while let Some(row) = rows.next().await.context("failed to fetch next row")? {
            buffer.push(f(&row)?);
        }
        Ok(buffer)
    }
}
