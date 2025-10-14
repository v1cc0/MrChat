use std::path::Path;

use anyhow::{Context, Result};
use std::fs;

use smol::block_on;
use tracing::warn;
use turso::{Connection, Database, params::IntoParams};
use turso_core::types::FromValue;

#[derive(Clone)]
pub struct TursoDatabase {
    inner: Database,
}

impl TursoDatabase {
    pub async fn open_local(path: impl AsRef<Path>) -> Result<Self> {
        let db = turso::Builder::new_local(path.as_ref().to_string_lossy().as_ref())
            .build()
            .await
            .context("failed to open local Turso database")?;

        let conn = db
            .connect()
            .context("failed to connect to turso database for pragma setup")?;
        conn.execute("PRAGMA journal_mode = WAL", ())
            .await
            .context("failed to enable WAL mode")?;

        Ok(Self { inner: db })
    }

    pub fn connect(&self) -> Result<TursoConnection> {
        let conn = self
            .inner
            .connect()
            .context("failed to connect to turso database")?;
        block_on(async { conn.execute("PRAGMA busy_timeout = 5000", ()).await })
            .context("failed to set busy timeout")?;
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

        let conn = self.connect()?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS mrchat_migrations (
                filename TEXT PRIMARY KEY,
                applied_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
            )",
            (),
        )
        .await
        .context("failed to ensure mrchat_migrations bookkeeping table")?;

        for entry in entries {
            let path = entry.path();
            let sql = fs::read_to_string(&path)
                .with_context(|| format!("failed to read migration {:?}", path))?;

            let filename = path
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .context("migration file missing filename")?;

            let already_applied = conn
                .query_scalar_optional::<i64>(
                    "SELECT 1 FROM mrchat_migrations WHERE filename = $1",
                    (filename.as_str(),),
                )
                .await
                .with_context(|| format!("failed to check migration history for {:?}", filename))?;

            if already_applied.is_some() {
                continue;
            }

            let execution = conn.execute_batch(sql.as_str()).await;

            match execution {
                Ok(()) => {}
                Err(err) => {
                    let mut benign = false;
                    let mut cause_message = String::new();

                    for cause in err.chain() {
                        let msg = cause.to_string();
                        if msg.contains("duplicate column name") || msg.contains("already exists") {
                            benign = true;
                            cause_message = msg;
                            break;
                        }

                        // Keep the deepest cause for diagnostics if nothing matches.
                        cause_message = msg;
                    }

                    if benign {
                        warn!(
                            "skipping migration {:?} because it appears already applied: {}",
                            filename, cause_message
                        );
                    } else {
                        return Err(err)
                            .context(format!("failed to execute migration {:?}", filename));
                    }
                }
            }

            conn.execute(
                "INSERT INTO mrchat_migrations (filename) VALUES ($1)",
                (filename.as_str(),),
            )
            .await
            .with_context(|| format!("failed to record migration {:?}", filename))?;
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

    pub async fn query_one<T, F>(&self, sql: &str, params: impl IntoParams, f: F) -> Result<T>
    where
        F: FnOnce(&turso::Row) -> Result<T>,
    {
        let mut rows = self.query(sql, params).await?;
        let row = rows
            .next()
            .await
            .context("failed to fetch row")?
            .context("no rows returned")?;
        f(&row)
    }

    pub async fn query_optional<T, F>(
        &self,
        sql: &str,
        params: impl IntoParams,
        f: F,
    ) -> Result<Option<T>>
    where
        F: FnOnce(&turso::Row) -> Result<T>,
    {
        let mut rows = self.query(sql, params).await?;
        match rows.next().await.context("failed to fetch row")? {
            Some(row) => Ok(Some(f(&row)?)),
            None => Ok(None),
        }
    }

    pub async fn query_scalar<T>(&self, sql: &str, params: impl IntoParams) -> Result<T>
    where
        T: FromValue,
    {
        let mut rows = self.query(sql, params).await?;
        let row = rows
            .next()
            .await
            .context("failed to fetch row")?
            .context("no rows returned")?;
        row.get(0).context("failed to get column 0")
    }

    pub async fn query_scalar_optional<T>(
        &self,
        sql: &str,
        params: impl IntoParams,
    ) -> Result<Option<T>>
    where
        T: FromValue,
    {
        let mut rows = self.query(sql, params).await?;
        match rows.next().await.context("failed to fetch row")? {
            Some(row) => Ok(Some(row.get(0).context("failed to get column 0")?)),
            None => Ok(None),
        }
    }

    /// Execute a query and return the last inserted row ID
    pub async fn execute_returning_id(&self, sql: &str, params: impl IntoParams) -> Result<i64> {
        self.execute(sql, params).await?;
        self.query_scalar::<i64>("SELECT last_insert_rowid()", ())
            .await
    }
}
