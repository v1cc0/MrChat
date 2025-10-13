use std::{env, path::PathBuf, sync::Arc};

use anyhow::{Context, Result};
use async_compat::CompatExt;
use libsql::{Builder, Connection as LibsqlConnection, Database, Rows, params::IntoParams};

/// Turso connection endpoint abstraction.
#[derive(Clone, Debug)]
pub enum TursoEndpoint {
    Remote { url: String, auth_token: String },
    File { path: PathBuf },
}

/// Configuration holder for establishing a Turso connection.
#[derive(Clone, Debug)]
pub struct TursoConfig {
    pub endpoint: TursoEndpoint,
}

impl TursoConfig {
    /// Build configuration from environment variables.
    ///
    /// Expected vars:
    /// - `TURSO_DATABASE_URL`: remote endpoint (libsql:// / https://) or local file path.
    /// - `TURSO_AUTH_TOKEN`: required when pointing to a remote service.
    pub fn from_env() -> Result<Self> {
        let raw_url = env::var("TURSO_DATABASE_URL")
            .context("TURSO_DATABASE_URL must be set to connect to Turso")?;

        if raw_url.starts_with("libsql://")
            || raw_url.starts_with("https://")
            || raw_url.starts_with("http://")
        {
            let auth_token = env::var("TURSO_AUTH_TOKEN").context(
                "TURSO_AUTH_TOKEN must be set when TURSO_DATABASE_URL points to a remote endpoint",
            )?;

            Ok(Self {
                endpoint: TursoEndpoint::Remote {
                    url: raw_url,
                    auth_token,
                },
            })
        } else {
            Ok(Self {
                endpoint: TursoEndpoint::File {
                    path: PathBuf::from(raw_url),
                },
            })
        }
    }
}

/// Lightweight wrapper around a libSQL [`Database`], providing async helpers.
#[derive(Clone)]
pub struct TursoPool {
    inner: Arc<Database>,
}

impl TursoPool {
    /// Establish a connection to Turso according to the provided config.
    pub async fn connect(config: TursoConfig) -> Result<Self> {
        let database = match config.endpoint {
            TursoEndpoint::Remote { url, auth_token } => Builder::new_remote(url, auth_token)
                .build()
                .compat()
                .await
                .context("failed to build remote Turso database handle")?,
            TursoEndpoint::File { path } => Builder::new_local(path)
                .build()
                .compat()
                .await
                .context("failed to build local Turso database handle")?,
        };

        Ok(Self {
            inner: Arc::new(database),
        })
    }

    /// Acquire a connection from the pool.
    pub fn connection(&self) -> Result<TursoConnection> {
        let conn = self
            .inner
            .connect()
            .context("failed to acquire Turso connection")?;

        Ok(TursoConnection { inner: conn })
    }

    /// Expose the underlying database for advanced use cases.
    pub fn raw(&self) -> Arc<Database> {
        self.inner.clone()
    }
}

/// Connection wrapper exposing async helpers with runtime compatibility.
#[derive(Clone)]
pub struct TursoConnection {
    inner: LibsqlConnection,
}

impl TursoConnection {
    pub async fn execute(&self, sql: &str, params: impl IntoParams) -> Result<u64> {
        self.inner
            .execute(sql, params)
            .compat()
            .await
            .context("failed to execute statement on Turso")
    }

    pub async fn query(&self, sql: &str, params: impl IntoParams) -> Result<Rows> {
        self.inner
            .query(sql, params)
            .compat()
            .await
            .context("failed to query Turso")
    }

    pub fn inner(&self) -> &LibsqlConnection {
        &self.inner
    }
}
