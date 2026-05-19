//! Shared SQLite schema introspection used by the optional `auto-rest`
//! and `graphql` surfaces.
//!
//! Build a [`SchemaCache`] once at boot via [`SchemaCache::introspect`],
//! then look up tables and columns by name. Both modules guard every
//! request through the cache so column / table names in URLs and GraphQL
//! arguments are *never* substituted as raw SQL fragments.
//!
//! See `crates/actant-server/src/auto_rest.rs` and `graphql_api.rs`.

#![cfg(any(feature = "auto-rest", feature = "graphql"))]

use std::collections::BTreeMap;

use actant_core::ActantError;
use sqlx::{Row, SqlitePool};

/// Tables that are append-only via the typed command layer and must NEVER
/// be exposed via auto-REST / GraphQL CRUD. The Chronicle's tamper-evidence
/// hinges on the engine being the only writer; ditto the per-command
/// idempotency log.
pub const APPEND_ONLY_TABLES: &[&str] = &["agent_event", "command_record"];

/// One column in a table.
#[derive(Debug, Clone)]
pub struct ColumnMeta {
    /// Column name.
    pub name: String,
    /// Affinity reported by SQLite (`TEXT`, `INTEGER`, `REAL`, `BLOB`,
    /// `NUMERIC`, `BOOLEAN`, etc.). Empty when SQLite reports no type.
    pub sql_type: String,
    /// `NOT NULL` flag.
    pub notnull: bool,
    /// Whether this column is part of the primary key (composite > 0).
    pub pk: bool,
}

/// One introspected table.
#[derive(Debug, Clone)]
pub struct TableMeta {
    /// Table name.
    pub name: String,
    /// Ordered columns.
    pub columns: Vec<ColumnMeta>,
    /// Cached column-name lookup for fast existence checks.
    pub column_names: std::collections::HashSet<String>,
    /// Whether this table has a `workspace_id` column; only tables that do
    /// are exposed via auto-REST / GraphQL (workspace isolation is a hard
    /// requirement).
    pub has_workspace_id: bool,
}

impl TableMeta {
    /// Validate that `name` is one of this table's columns.
    pub fn has_column(&self, name: &str) -> bool {
        self.column_names.contains(name)
    }

    /// Borrow the column metadata if it exists.
    pub fn column(&self, name: &str) -> Option<&ColumnMeta> {
        self.columns.iter().find(|c| c.name == name)
    }
}

/// All tables exposable via auto-REST / GraphQL, keyed by name.
#[derive(Debug, Clone, Default)]
pub struct SchemaCache {
    /// BTreeMap so listings are deterministic.
    pub tables: BTreeMap<String, TableMeta>,
}

impl SchemaCache {
    /// Introspect the current SQLite schema, filtering out internal,
    /// append-only, and workspace-less tables.
    pub async fn introspect(pool: &SqlitePool) -> Result<Self, ActantError> {
        // List user tables.
        let names: Vec<(String,)> = sqlx::query_as(
            "SELECT name FROM sqlite_master
             WHERE type = 'table'
               AND name NOT LIKE 'sqlite_%'
               AND name NOT LIKE 'schema_%'
             ORDER BY name",
        )
        .fetch_all(pool)
        .await
        .map_err(map_sqlx)?;

        let mut tables: BTreeMap<String, TableMeta> = BTreeMap::new();
        for (name,) in names {
            if APPEND_ONLY_TABLES.contains(&name.as_str()) {
                continue;
            }
            // PRAGMA table_info returns: cid, name, type, notnull, dflt_value, pk
            let rows = sqlx::query(&format!("PRAGMA table_info(\"{}\")", escape_ident(&name)))
                .fetch_all(pool)
                .await
                .map_err(map_sqlx)?;
            let mut columns = Vec::with_capacity(rows.len());
            let mut column_names = std::collections::HashSet::with_capacity(rows.len());
            for r in rows {
                let col_name: String = r.try_get("name").map_err(map_sqlx)?;
                let sql_type: String = r.try_get("type").map_err(map_sqlx).unwrap_or_default();
                let notnull: i64 = r.try_get("notnull").map_err(map_sqlx).unwrap_or(0);
                let pk: i64 = r.try_get("pk").map_err(map_sqlx).unwrap_or(0);
                column_names.insert(col_name.clone());
                columns.push(ColumnMeta {
                    name: col_name,
                    sql_type,
                    notnull: notnull != 0,
                    pk: pk != 0,
                });
            }
            let has_workspace_id = column_names.contains("workspace_id");
            // Tables without a workspace_id are infrastructure (workspace
            // itself, schema_migration mirror, etc.) and can't be exposed
            // safely via the workspace-scoped REST/GraphQL surface.
            if !has_workspace_id && name != "workspace" {
                continue;
            }
            tables.insert(
                name.clone(),
                TableMeta {
                    name,
                    columns,
                    column_names,
                    has_workspace_id,
                },
            );
        }
        Ok(SchemaCache { tables })
    }

    /// Borrow a table's metadata.
    pub fn table(&self, name: &str) -> Option<&TableMeta> {
        self.tables.get(name)
    }

    /// Names of every exposable table.
    pub fn table_names(&self) -> Vec<String> {
        self.tables.keys().cloned().collect()
    }
}

/// Defense in depth: escape an identifier the cache verified. The cache
/// already guarantees the string came from sqlite_master, but we double-
/// escape on every use site so future code paths that hand in
/// caller-controlled strings can't open a hole.
pub fn escape_ident(s: &str) -> String {
    s.replace('"', "\"\"")
}

fn map_sqlx(e: sqlx::Error) -> ActantError {
    ActantError::Storage(e.to_string())
}
