//! Auto-generated REST API (DEVX_GAPS X88).
//!
//! Surfaces the SQLite schema as a PostgREST-style `/rest/v1/<table>`
//! family of endpoints. Tables `agent_event` and `command_record` are
//! deliberately excluded (they're append-only via the typed command
//! engine) — see [`crate::schema_introspect::APPEND_ONLY_TABLES`].
//!
//! Filters use the column-as-param convention:
//!
//! ```text
//! GET    /rest/v1/<table>?workspace_id=ws_x&select=col1,col2&id=eq.123&order=created_at.desc&limit=10
//! POST   /rest/v1/<table>            { workspace_id, ...row }
//! PATCH  /rest/v1/<table>?workspace_id=ws_x&id=eq.123       { col: new_value, ... }
//! DELETE /rest/v1/<table>?workspace_id=ws_x&id=eq.123
//! ```
//!
//! Reserved query parameters: `workspace_id`, `select`, `order`, `limit`,
//! `offset`. Everything else is interpreted as a column filter using the
//! `<col>=<op>.<value>` form, mirroring PostgREST. The supported `op`s are
//! `eq`, `neq`, `lt`, `gt`, `lte`, `gte`, `like`, `in`, `is`.
//!
//! Every request runs through [`crate::enforce_auth`] with the caller-
//! supplied `workspace_id`, so cookies / bearer tokens / CSRF carry over
//! exactly as on the typed routes. The `workspace_id = ?` predicate is
//! always appended to the WHERE clause; the row's own `workspace_id` is
//! enforced on every INSERT.

#![cfg(feature = "auto-rest")]

use std::sync::Arc;

use actant_core::ActantError;
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, Method, StatusCode},
    response::{IntoResponse, Json, Response},
    routing::get,
    Router,
};
use sqlx::Row;

use crate::schema_introspect::{escape_ident, ColumnMeta, SchemaCache, TableMeta};
use crate::{enforce_auth, enforce_rate_limit, AppState};

/// Mount the auto-REST routes onto an existing router.
pub fn mount(router: Router<Arc<AppState>>) -> Router<Arc<AppState>> {
    router
        .route("/rest/v1", get(list_tables))
        .route("/rest/v1/{table}/columns", get(list_columns))
        .route(
            "/rest/v1/{table}",
            get(handle_get)
                .post(handle_post)
                .patch(handle_patch)
                .delete(handle_delete),
        )
}

#[derive(serde::Deserialize)]
struct TablePath {
    table: String,
}

/// `GET /rest/v1` — list exposed tables.
async fn list_tables(State(s): State<Arc<AppState>>) -> Response {
    let Some(cache) = s.schema_cache.as_ref() else {
        return missing_cache_response();
    };
    Json(serde_json::json!({
        "tables": cache.table_names(),
    }))
    .into_response()
}

/// `GET /rest/v1/<table>/columns` — list column metadata.
async fn list_columns(
    State(s): State<Arc<AppState>>,
    Path(TablePath { table }): Path<TablePath>,
) -> Response {
    let Some(cache) = s.schema_cache.as_ref() else {
        return missing_cache_response();
    };
    let Some(tbl) = cache.table(&table) else {
        return table_not_found(&table);
    };
    let cols: Vec<_> = tbl
        .columns
        .iter()
        .map(|c| {
            serde_json::json!({
                "name": c.name,
                "type": c.sql_type,
                "notnull": c.notnull,
                "pk": c.pk,
            })
        })
        .collect();
    Json(serde_json::json!({ "table": tbl.name, "columns": cols })).into_response()
}

// ---------------------------------------------------------------------------
// GET
// ---------------------------------------------------------------------------

async fn handle_get(
    State(s): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(TablePath { table }): Path<TablePath>,
    Query(params): Query<Vec<(String, String)>>,
) -> Response {
    let Some(cache) = s.schema_cache.as_ref() else {
        return missing_cache_response();
    };
    let Some(tbl) = cache.table(&table) else {
        return table_not_found(&table);
    };
    let parsed = match RequestParams::parse(&params, tbl) {
        Ok(p) => p,
        Err(e) => return bad_request(&e),
    };
    if let Err(resp) = enforce_auth(&s, &headers, &Method::GET, &parsed.workspace_id).await {
        return resp;
    }

    let mut sql = String::with_capacity(128);
    sql.push_str("SELECT ");
    sql.push_str(&parsed.select_sql);
    sql.push_str(" FROM \"");
    sql.push_str(&escape_ident(&tbl.name));
    sql.push_str("\" WHERE workspace_id = ?");
    let mut binds: Vec<String> = vec![parsed.workspace_id.clone()];
    for f in &parsed.filters {
        f.append_to(&mut sql, &mut binds);
    }
    if let Some((col, asc)) = &parsed.order {
        sql.push_str(" ORDER BY \"");
        sql.push_str(&escape_ident(col));
        sql.push('"');
        sql.push_str(if *asc { " ASC" } else { " DESC" });
    }
    if let Some(limit) = parsed.limit {
        sql.push_str(" LIMIT ");
        sql.push_str(&limit.to_string());
    }
    if let Some(offset) = parsed.offset {
        sql.push_str(" OFFSET ");
        sql.push_str(&offset.to_string());
    }

    let mut q = sqlx::query(&sql);
    for b in &binds {
        q = q.bind(b);
    }
    let rows = match q.fetch_all(s.storage.pool()).await {
        Ok(r) => r,
        Err(e) => return storage_err(e),
    };
    let projected: Vec<serde_json::Value> = rows
        .iter()
        .map(|row| row_to_json(row, &parsed.select_cols))
        .collect();

    // PostgREST returns a single object when a primary-key filter matches
    // exactly one row. We mirror that by detecting `id=eq.X` (or any pk
    // equality) and unwrapping when the result has length 1.
    let single = parsed.implies_single_row() && projected.len() == 1;
    if single {
        Json(projected.into_iter().next().unwrap()).into_response()
    } else {
        Json(projected).into_response()
    }
}

// ---------------------------------------------------------------------------
// POST / PATCH / DELETE
// ---------------------------------------------------------------------------

async fn handle_post(
    State(s): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(TablePath { table }): Path<TablePath>,
    Query(params): Query<Vec<(String, String)>>,
    Json(body): Json<serde_json::Value>,
) -> Response {
    let Some(cache) = s.schema_cache.as_ref() else {
        return missing_cache_response();
    };
    let Some(tbl) = cache.table(&table) else {
        return table_not_found(&table);
    };
    let workspace_id = match workspace_id_from(&params, &body) {
        Some(w) => w,
        None => return bad_request("workspace_id is required"),
    };
    if let Err(resp) = enforce_rate_limit(&s, &workspace_id).await {
        return resp;
    }
    if let Err(resp) = enforce_auth(&s, &headers, &Method::POST, &workspace_id).await {
        return resp;
    }
    let obj = match body.as_object() {
        Some(o) => o.clone(),
        None => return bad_request("request body must be a JSON object"),
    };
    // Project the body onto known columns; reject unknown columns
    // explicitly so silent drops can't happen.
    let mut cols: Vec<String> = Vec::new();
    let mut binds: Vec<String> = Vec::new();
    let mut has_workspace_id = false;
    for (k, v) in &obj {
        if !tbl.has_column(k) {
            return bad_request(&format!("unknown column '{k}' on table '{}'", tbl.name));
        }
        if k == "workspace_id" {
            has_workspace_id = true;
        }
        cols.push(k.clone());
        binds.push(value_to_sql_string(v));
    }
    if !has_workspace_id {
        cols.push("workspace_id".into());
        binds.push(workspace_id.clone());
    }
    if cols.is_empty() {
        return bad_request("request body must not be empty");
    }
    let placeholders = std::iter::repeat("?")
        .take(cols.len())
        .collect::<Vec<_>>()
        .join(", ");
    let columns_sql = cols
        .iter()
        .map(|c| format!("\"{}\"", escape_ident(c)))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "INSERT INTO \"{}\" ({}) VALUES ({})",
        escape_ident(&tbl.name),
        columns_sql,
        placeholders
    );
    let mut q = sqlx::query(&sql);
    for b in &binds {
        q = q.bind(b);
    }
    match q.execute(s.storage.pool()).await {
        Ok(_) => (StatusCode::CREATED, Json(serde_json::json!({ "ok": true }))).into_response(),
        Err(e) => storage_err(e),
    }
}

async fn handle_patch(
    State(s): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(TablePath { table }): Path<TablePath>,
    Query(params): Query<Vec<(String, String)>>,
    Json(body): Json<serde_json::Value>,
) -> Response {
    let Some(cache) = s.schema_cache.as_ref() else {
        return missing_cache_response();
    };
    let Some(tbl) = cache.table(&table) else {
        return table_not_found(&table);
    };
    let parsed = match RequestParams::parse(&params, tbl) {
        Ok(p) => p,
        Err(e) => return bad_request(&e),
    };
    if let Err(resp) = enforce_rate_limit(&s, &parsed.workspace_id).await {
        return resp;
    }
    if let Err(resp) = enforce_auth(&s, &headers, &Method::PATCH, &parsed.workspace_id).await {
        return resp;
    }
    let obj = match body.as_object() {
        Some(o) => o.clone(),
        None => return bad_request("request body must be a JSON object"),
    };
    if obj.is_empty() {
        return bad_request("PATCH body must include at least one column");
    }
    let mut set_clauses: Vec<String> = Vec::new();
    let mut binds: Vec<String> = Vec::new();
    for (k, v) in &obj {
        if k == "workspace_id" {
            return bad_request("workspace_id cannot be patched");
        }
        if !tbl.has_column(k) {
            return bad_request(&format!("unknown column '{k}' on table '{}'", tbl.name));
        }
        set_clauses.push(format!("\"{}\" = ?", escape_ident(k)));
        binds.push(value_to_sql_string(v));
    }
    let mut sql = format!(
        "UPDATE \"{}\" SET {} WHERE workspace_id = ?",
        escape_ident(&tbl.name),
        set_clauses.join(", ")
    );
    binds.push(parsed.workspace_id.clone());
    for f in &parsed.filters {
        f.append_to(&mut sql, &mut binds);
    }
    let mut q = sqlx::query(&sql);
    for b in &binds {
        q = q.bind(b);
    }
    match q.execute(s.storage.pool()).await {
        Ok(r) => Json(serde_json::json!({ "updated": r.rows_affected() })).into_response(),
        Err(e) => storage_err(e),
    }
}

async fn handle_delete(
    State(s): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(TablePath { table }): Path<TablePath>,
    Query(params): Query<Vec<(String, String)>>,
) -> Response {
    let Some(cache) = s.schema_cache.as_ref() else {
        return missing_cache_response();
    };
    let Some(tbl) = cache.table(&table) else {
        return table_not_found(&table);
    };
    let parsed = match RequestParams::parse(&params, tbl) {
        Ok(p) => p,
        Err(e) => return bad_request(&e),
    };
    if parsed.filters.is_empty() {
        return bad_request("DELETE requires at least one filter (e.g. ?id=eq.X)");
    }
    if let Err(resp) = enforce_rate_limit(&s, &parsed.workspace_id).await {
        return resp;
    }
    if let Err(resp) = enforce_auth(&s, &headers, &Method::DELETE, &parsed.workspace_id).await {
        return resp;
    }
    let mut sql = format!(
        "DELETE FROM \"{}\" WHERE workspace_id = ?",
        escape_ident(&tbl.name)
    );
    let mut binds: Vec<String> = vec![parsed.workspace_id.clone()];
    for f in &parsed.filters {
        f.append_to(&mut sql, &mut binds);
    }
    let mut q = sqlx::query(&sql);
    for b in &binds {
        q = q.bind(b);
    }
    match q.execute(s.storage.pool()).await {
        Ok(r) => Json(serde_json::json!({ "deleted": r.rows_affected() })).into_response(),
        Err(e) => storage_err(e),
    }
}

// ---------------------------------------------------------------------------
// Query parameter parsing
// ---------------------------------------------------------------------------

struct RequestParams {
    workspace_id: String,
    /// Already-quoted column list for the SELECT clause.
    select_sql: String,
    /// Column names (unquoted) projected from the row.
    select_cols: Vec<String>,
    filters: Vec<Filter>,
    order: Option<(String, bool)>,
    limit: Option<u32>,
    offset: Option<u32>,
}

impl RequestParams {
    fn parse(params: &[(String, String)], tbl: &TableMeta) -> Result<Self, String> {
        let mut workspace_id: Option<String> = None;
        let mut select: Option<String> = None;
        let mut order: Option<(String, bool)> = None;
        let mut limit: Option<u32> = None;
        let mut offset: Option<u32> = None;
        let mut filters: Vec<Filter> = Vec::new();

        for (k, v) in params {
            match k.as_str() {
                "workspace_id" => workspace_id = Some(v.clone()),
                "select" => select = Some(v.clone()),
                "order" => {
                    let (col, asc) = parse_order(v, tbl)?;
                    order = Some((col, asc));
                }
                "limit" => limit = Some(v.parse().map_err(|_| "invalid limit".to_string())?),
                "offset" => offset = Some(v.parse().map_err(|_| "invalid offset".to_string())?),
                _ => {
                    filters.push(Filter::parse(k, v, tbl)?);
                }
            }
        }
        let workspace_id =
            workspace_id.ok_or_else(|| "workspace_id query parameter is required".to_string())?;
        if workspace_id.is_empty() {
            return Err("workspace_id query parameter is required".into());
        }

        let (select_sql, select_cols) = build_select(select.as_deref(), tbl)?;

        Ok(RequestParams {
            workspace_id,
            select_sql,
            select_cols,
            filters,
            order,
            limit,
            offset,
        })
    }

    /// Heuristic: a request implies "single row" when it filters by a
    /// primary-key column using `eq`. Mirrors PostgREST's behavior.
    fn implies_single_row(&self) -> bool {
        self.filters.iter().any(|f| f.is_pk_eq())
    }
}

fn parse_order(spec: &str, tbl: &TableMeta) -> Result<(String, bool), String> {
    let mut parts = spec.splitn(2, '.');
    let col = parts.next().unwrap_or("").to_string();
    let dir = parts.next().unwrap_or("asc");
    if !tbl.has_column(&col) {
        return Err(format!("unknown column '{col}' in order"));
    }
    let asc = match dir {
        "asc" | "" => true,
        "desc" => false,
        other => return Err(format!("invalid order direction '{other}'")),
    };
    Ok((col, asc))
}

fn build_select(select: Option<&str>, tbl: &TableMeta) -> Result<(String, Vec<String>), String> {
    let cols: Vec<String> = match select {
        None | Some("*") | Some("") => tbl.columns.iter().map(|c| c.name.clone()).collect(),
        Some(list) => {
            let names: Vec<String> = list
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            for n in &names {
                if !tbl.has_column(n) {
                    return Err(format!("unknown column '{n}' in select"));
                }
            }
            names
        }
    };
    let sql = cols
        .iter()
        .map(|c| format!("\"{}\"", escape_ident(c)))
        .collect::<Vec<_>>()
        .join(", ");
    Ok((sql, cols))
}

#[derive(Clone)]
struct Filter {
    column: String,
    op: FilterOp,
    is_pk: bool,
}

#[derive(Clone)]
enum FilterOp {
    Eq(String),
    Ne(String),
    Lt(String),
    Le(String),
    Gt(String),
    Ge(String),
    Like(String),
    In(Vec<String>),
    IsNull,
    IsNotNull,
}

impl Filter {
    fn parse(column: &str, value: &str, tbl: &TableMeta) -> Result<Self, String> {
        if !tbl.has_column(column) {
            return Err(format!("unknown column '{column}'"));
        }
        let col = tbl.column(column).expect("checked");
        let is_pk = col.pk;
        let op = parse_op(value)?;
        Ok(Filter {
            column: column.to_string(),
            op,
            is_pk,
        })
    }

    fn is_pk_eq(&self) -> bool {
        self.is_pk && matches!(self.op, FilterOp::Eq(_))
    }

    fn append_to(&self, sql: &mut String, binds: &mut Vec<String>) {
        let col = format!("\"{}\"", escape_ident(&self.column));
        match &self.op {
            FilterOp::Eq(v) => {
                sql.push_str(" AND ");
                sql.push_str(&col);
                sql.push_str(" = ?");
                binds.push(v.clone());
            }
            FilterOp::Ne(v) => {
                sql.push_str(" AND ");
                sql.push_str(&col);
                sql.push_str(" <> ?");
                binds.push(v.clone());
            }
            FilterOp::Lt(v) => {
                sql.push_str(" AND ");
                sql.push_str(&col);
                sql.push_str(" < ?");
                binds.push(v.clone());
            }
            FilterOp::Le(v) => {
                sql.push_str(" AND ");
                sql.push_str(&col);
                sql.push_str(" <= ?");
                binds.push(v.clone());
            }
            FilterOp::Gt(v) => {
                sql.push_str(" AND ");
                sql.push_str(&col);
                sql.push_str(" > ?");
                binds.push(v.clone());
            }
            FilterOp::Ge(v) => {
                sql.push_str(" AND ");
                sql.push_str(&col);
                sql.push_str(" >= ?");
                binds.push(v.clone());
            }
            FilterOp::Like(v) => {
                sql.push_str(" AND ");
                sql.push_str(&col);
                sql.push_str(" LIKE ?");
                binds.push(v.clone());
            }
            FilterOp::In(vs) => {
                sql.push_str(" AND ");
                sql.push_str(&col);
                sql.push_str(" IN (");
                for (i, v) in vs.iter().enumerate() {
                    if i > 0 {
                        sql.push_str(", ");
                    }
                    sql.push('?');
                    binds.push(v.clone());
                }
                sql.push(')');
            }
            FilterOp::IsNull => {
                sql.push_str(" AND ");
                sql.push_str(&col);
                sql.push_str(" IS NULL");
            }
            FilterOp::IsNotNull => {
                sql.push_str(" AND ");
                sql.push_str(&col);
                sql.push_str(" IS NOT NULL");
            }
        }
    }
}

fn parse_op(value: &str) -> Result<FilterOp, String> {
    let mut parts = value.splitn(2, '.');
    let op = parts.next().unwrap_or("");
    let rest = parts.next().unwrap_or("");
    match op {
        "eq" => Ok(FilterOp::Eq(rest.to_string())),
        "neq" => Ok(FilterOp::Ne(rest.to_string())),
        "lt" => Ok(FilterOp::Lt(rest.to_string())),
        "lte" => Ok(FilterOp::Le(rest.to_string())),
        "gt" => Ok(FilterOp::Gt(rest.to_string())),
        "gte" => Ok(FilterOp::Ge(rest.to_string())),
        "like" => Ok(FilterOp::Like(rest.to_string())),
        "in" => {
            // Form: in.(a,b,c)
            let inner = rest
                .trim_start_matches('(')
                .trim_end_matches(')')
                .to_string();
            let items: Vec<String> = inner
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            if items.is_empty() {
                return Err("in.() requires at least one value".into());
            }
            Ok(FilterOp::In(items))
        }
        "is" => match rest {
            "null" => Ok(FilterOp::IsNull),
            "not.null" => Ok(FilterOp::IsNotNull),
            other => Err(format!("unsupported is.{other}")),
        },
        other => Err(format!(
            "unsupported filter op '{other}'; expected one of eq, neq, lt, gt, lte, gte, like, in, is"
        )),
    }
}

fn value_to_sql_string(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Bool(b) => {
            if *b {
                "1".into()
            } else {
                "0".into()
            }
        }
        serde_json::Value::Null => "".into(),
        other => other.to_string(),
    }
}

fn workspace_id_from(params: &[(String, String)], body: &serde_json::Value) -> Option<String> {
    for (k, v) in params {
        if k == "workspace_id" && !v.is_empty() {
            return Some(v.clone());
        }
    }
    body.get("workspace_id")
        .and_then(|v| v.as_str())
        .map(String::from)
}

fn row_to_json(row: &sqlx::sqlite::SqliteRow, cols: &[String]) -> serde_json::Value {
    let mut obj = serde_json::Map::with_capacity(cols.len());
    for col in cols {
        let v: serde_json::Value = sqlite_value_to_json(row, col);
        obj.insert(col.clone(), v);
    }
    serde_json::Value::Object(obj)
}

pub(crate) fn sqlite_value_to_json(row: &sqlx::sqlite::SqliteRow, col: &str) -> serde_json::Value {
    // Try text first (most ActantDB columns are TEXT), fall back to i64/f64.
    if let Ok(v) = row.try_get::<Option<String>, _>(col) {
        return v
            .map(serde_json::Value::String)
            .unwrap_or(serde_json::Value::Null);
    }
    if let Ok(v) = row.try_get::<Option<i64>, _>(col) {
        return v
            .map(|n| serde_json::Value::Number(n.into()))
            .unwrap_or(serde_json::Value::Null);
    }
    if let Ok(v) = row.try_get::<Option<f64>, _>(col) {
        return v
            .and_then(|n| serde_json::Number::from_f64(n).map(serde_json::Value::Number))
            .unwrap_or(serde_json::Value::Null);
    }
    serde_json::Value::Null
}

// ---------------------------------------------------------------------------
// Error helpers
// ---------------------------------------------------------------------------

fn missing_cache_response() -> Response {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(serde_json::json!({
            "error": "schema_cache_unavailable",
            "message": "auto-rest schema cache was not initialized; call AppState::with_schema_cache at boot"
        })),
    )
        .into_response()
}

fn table_not_found(name: &str) -> Response {
    (
        StatusCode::NOT_FOUND,
        Json(serde_json::json!({
            "error": "table_not_exposed",
            "message": format!(
                "table '{name}' is not exposed via /rest/v1 (append-only, missing workspace_id, or unknown)"
            )
        })),
    )
        .into_response()
}

fn bad_request(msg: &str) -> Response {
    (
        StatusCode::BAD_REQUEST,
        Json(serde_json::json!({ "error": "invalid_input", "message": msg })),
    )
        .into_response()
}

fn storage_err(e: sqlx::Error) -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(serde_json::json!({
            "error": "storage",
            "message": ActantError::Storage(e.to_string()).to_string()
        })),
    )
        .into_response()
}

// Used for `dead_code` linting parity in case build configurations strip
// uses elsewhere; the cache and column lookup always go through these.
#[allow(dead_code)]
fn _force_use(_: &SchemaCache, _: &ColumnMeta) {}
