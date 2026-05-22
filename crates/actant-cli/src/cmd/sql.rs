//! `actantdb sql <statement>` — read-only SQL prompt against the ledger.

use std::path::Path;
use std::str::FromStr;

use comfy_table::Table;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Column, Row, TypeInfo};

use crate::cli_errors;

/// Run the SQL command.
pub async fn run(db_path: &Path, statement: &str) -> anyhow::Result<()> {
    ensure_read_only(statement)?;

    // Open a strictly read-only connection — belt and braces against any
    // bug in `ensure_read_only`.
    let opts = SqliteConnectOptions::from_str(&format!("sqlite://{}", db_path.display()))
        .map_err(|e| cli_errors::storage("open read-only sqlite", e))?
        .read_only(true)
        .create_if_missing(false);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await
        .map_err(|e| cli_errors::storage("connect read-only sqlite", e))?;

    let rows = sqlx::query(statement)
        .fetch_all(&pool)
        .await
        .map_err(|e| cli_errors::storage("run read-only query", e))?;
    if rows.is_empty() {
        println!("(no rows)");
        return Ok(());
    }

    let mut table = Table::new();
    let header: Vec<String> = rows[0]
        .columns()
        .iter()
        .map(|c| c.name().to_string())
        .collect();
    table.set_header(header.clone());

    for r in &rows {
        let mut cells = Vec::with_capacity(header.len());
        for (idx, c) in r.columns().iter().enumerate() {
            cells.push(stringify_cell(r, idx, c.type_info().name()));
        }
        table.add_row(cells);
    }
    println!("{table}");
    println!(
        "({} row{})",
        rows.len(),
        if rows.len() == 1 { "" } else { "s" }
    );
    Ok(())
}

fn stringify_cell(row: &sqlx::sqlite::SqliteRow, idx: usize, type_name: &str) -> String {
    // We try the most common SQLite-mapped Rust types in order. Anything
    // we can't read renders as "<?>".
    if let Ok(v) = row.try_get::<Option<String>, _>(idx) {
        return v.unwrap_or_default();
    }
    if let Ok(v) = row.try_get::<Option<i64>, _>(idx) {
        return v.map(|n| n.to_string()).unwrap_or_default();
    }
    if let Ok(v) = row.try_get::<Option<f64>, _>(idx) {
        return v.map(|n| n.to_string()).unwrap_or_default();
    }
    if let Ok(v) = row.try_get::<Option<Vec<u8>>, _>(idx) {
        return v
            .map(|b| format!("<{} bytes>", b.len()))
            .unwrap_or_default();
    }
    format!("<? {type_name}>")
}

/// Permit only SELECT / WITH statements. Rejects compound statements (we
/// don't allow `;` outside of trailing whitespace).
pub fn ensure_read_only(stmt: &str) -> anyhow::Result<()> {
    let trimmed = stmt.trim();
    let body = trimmed.trim_end_matches(';').trim();
    let first_word = body
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_ascii_uppercase();
    if first_word != "SELECT" && first_word != "WITH" {
        return Err(cli_errors::invalid_input(format!(
            "only SELECT / WITH statements are allowed in `actantdb sql`; got `{first_word}`"
        ))
        .into());
    }
    // Refuse multiple statements.
    let body_no_strings = strip_string_literals(body);
    if body_no_strings.contains(';') {
        return Err(cli_errors::invalid_input("multiple statements are not allowed").into());
    }
    Ok(())
}

fn strip_string_literals(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_str = false;
    let mut quote = '\0';
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if in_str {
            if c == quote {
                // Handle SQL doubled-quote escape: 'it''s'
                if chars.peek() == Some(&quote) {
                    chars.next();
                    continue;
                }
                in_str = false;
            }
            // skip char (don't push)
        } else if c == '\'' || c == '"' {
            in_str = true;
            quote = c;
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_select() {
        ensure_read_only("SELECT 1").unwrap();
        ensure_read_only("  select * from agent_event;").unwrap();
        ensure_read_only("WITH x AS (SELECT 1) SELECT * FROM x").unwrap();
    }
    #[test]
    fn rejects_dml() {
        for stmt in [
            "INSERT INTO foo VALUES (1)",
            "delete from x",
            "DROP TABLE agent_event",
            "ATTACH DATABASE 'x' AS y",
        ] {
            assert!(ensure_read_only(stmt).is_err());
        }
    }
    #[test]
    fn rejects_multiple() {
        assert!(ensure_read_only("SELECT 1; DELETE FROM x").is_err());
    }
    #[test]
    fn allows_semicolon_in_string_literal() {
        ensure_read_only("SELECT 'a;b'").unwrap();
    }
}
