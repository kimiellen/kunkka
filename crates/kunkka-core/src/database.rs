use crate::xdg::KunkkaPaths;
use crate::{CoreError, Result};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};
use std::str::FromStr;

pub struct CoreDatabase {
    pool: SqlitePool,
}

impl CoreDatabase {
    pub async fn connect(paths: &KunkkaPaths) -> Result<Self> {
        if let Some(parent) = paths.database_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let options = SqliteConnectOptions::from_str(&paths.database_path.to_string_lossy())
            .map_err(|err| CoreError::Database(format!("invalid database path: {err}")))?
            .create_if_missing(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
            .pragma("foreign_keys", "ON");

        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .map_err(|err| CoreError::Database(format!("failed to connect: {err}")))?;

        let db = Self { pool };
        db.run_migrations().await?;
        Ok(db)
    }

    pub async fn schema_version(&self) -> Result<i64> {
        let row = sqlx::query("SELECT value FROM core_metadata WHERE key = 'schema_version'")
            .fetch_optional(&self.pool)
            .await
            .map_err(|err| CoreError::Database(format!("failed to query schema_version: {err}")))?
            .ok_or_else(|| CoreError::Database("schema_version not found".to_string()))?;

        let value: String = row
            .try_get("value")
            .map_err(|err| CoreError::Database(format!("failed to read schema_version: {err}")))?;

        value
            .parse::<i64>()
            .map_err(|_| CoreError::Database(format!("invalid schema_version: {value}")))
    }

    pub async fn ping(&self) -> Result<()> {
        sqlx::query("SELECT 1")
            .execute(&self.pool)
            .await
            .map_err(|err| CoreError::Database(format!("ping failed: {err}")))?;
        Ok(())
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    async fn run_migrations(&self) -> Result<()> {
        sqlx::migrate!()
            .run(&self.pool)
            .await
            .map_err(|err| CoreError::Database(format!("migration failed: {err}")))?;
        Ok(())
    }
}
