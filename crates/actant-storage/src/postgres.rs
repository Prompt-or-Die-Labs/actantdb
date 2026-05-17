//! Postgres backend for `actant-storage`.
//!
//! Phase 6. Mirrors the public `Storage` API but uses `sqlx::PgPool`.
//! Schema lives in `/migrations/pg/*.sql`.

use std::str::FromStr;

use actant_core::ActantError;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::PgPool;

const PG_MIGRATIONS: &[(&str, &str)] = &[(
    "pg_0001_initial",
    include_str!("../../../migrations/pg/0001_initial.sql"),
)];

/// Postgres storage handle.
#[derive(Debug, Clone)]
pub struct PgStorage {
    pool: PgPool,
}

impl PgStorage {
    /// Open a Postgres connection from a `postgres://user:pass@host/db` URL.
    pub async fn open(url: &str) -> Result<Self, ActantError> {
        let opts =
            PgConnectOptions::from_str(url).map_err(|e| ActantError::Storage(e.to_string()))?;
        let pool = PgPoolOptions::new()
            .max_connections(8)
            .connect_with(opts)
            .await
            .map_err(|e| ActantError::Storage(e.to_string()))?;
        let s = PgStorage { pool };
        s.run_migrations().await?;
        Ok(s)
    }

    /// Underlying pool.
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Apply embedded Postgres migrations.
    pub async fn run_migrations(&self) -> Result<(), ActantError> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS schema_migration (
                name TEXT PRIMARY KEY,
                applied_at TIMESTAMPTZ NOT NULL DEFAULT now()
            )",
        )
        .execute(&self.pool)
        .await
        .map_err(|e| ActantError::Storage(e.to_string()))?;

        for (name, sql) in PG_MIGRATIONS {
            let already: Option<(String,)> =
                sqlx::query_as("SELECT name FROM schema_migration WHERE name = $1")
                    .bind(name)
                    .fetch_optional(&self.pool)
                    .await
                    .map_err(|e| ActantError::Storage(e.to_string()))?;
            if already.is_some() {
                continue;
            }
            for raw in strip_comments(sql).split(';') {
                let stmt = raw.trim();
                if stmt.is_empty() {
                    continue;
                }
                sqlx::query(stmt).execute(&self.pool).await.map_err(|e| {
                    ActantError::Storage(format!("apply pg `{}`: {}", first_line(stmt), e))
                })?;
            }
            sqlx::query("INSERT INTO schema_migration (name) VALUES ($1)")
                .bind(name)
                .execute(&self.pool)
                .await
                .map_err(|e| ActantError::Storage(e.to_string()))?;
        }
        Ok(())
    }

    /// Names of every applied migration.
    pub async fn applied_migrations(&self) -> Result<Vec<String>, ActantError> {
        let rows: Vec<(String,)> =
            sqlx::query_as("SELECT name FROM schema_migration ORDER BY name")
                .fetch_all(&self.pool)
                .await
                .map_err(|e| ActantError::Storage(e.to_string()))?;
        Ok(rows.into_iter().map(|(n,)| n).collect())
    }
}

fn strip_comments(s: &str) -> String {
    let mut out = String::new();
    for line in s.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("--") {
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

fn first_line(s: &str) -> &str {
    s.lines().next().unwrap_or("").trim()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Connects to a Postgres if `ACTANTDB_TEST_PG_URL` is set; otherwise
    /// skips. CI must set this against a sidecar (see deploy/docker/docker-compose).
    #[tokio::test]
    async fn opens_and_applies_pg_schema() {
        let Ok(url) = std::env::var("ACTANTDB_TEST_PG_URL") else {
            eprintln!("skipped: set ACTANTDB_TEST_PG_URL to enable");
            return;
        };
        let s = PgStorage::open(&url).await.expect("open");
        let applied = s.applied_migrations().await.expect("applied");
        assert!(applied.contains(&"pg_0001_initial".to_string()));
    }
}
