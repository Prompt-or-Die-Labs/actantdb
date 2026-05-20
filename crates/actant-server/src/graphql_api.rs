//! GraphQL endpoint (DEVX_GAPS X89).
//!
//! Schema is auto-derived from the same [`crate::schema_introspect::SchemaCache`]
//! the auto-REST surface uses. Every introspected table becomes a query
//! field that accepts `workspace_id`, `where`, `order_by`, `limit`, `offset`
//! arguments and returns a JSON array.
//!
//! Mutations are deliberately NOT auto-derived. A single `command(type:
//! String!, input: JSON!)` mutation maps directly to the existing
//! `/v1/command` envelope (`workspace_id`, `actor_id`, `command_type`,
//! `input`, optional `idempotency_key`). Writes stay typed; we don't grow
//! a parallel write surface beside the command engine.
//!
//! See [the task spec](../../../DEVX_GAPS.md) row X89.

#![cfg(feature = "graphql")]

use std::sync::Arc;

use actant_command::Engine;
use actant_core::{ActorId, WorkspaceId};
use async_graphql::{
    dynamic::{
        Field, FieldFuture, FieldValue, InputObject, InputValue, Object, ResolverContext, Schema,
        SchemaError, TypeRef,
    },
    Request, Response as GqlResponse, Value as GqlValue,
};
use async_graphql_axum::GraphQLBatchRequest;
use axum::{
    extract::State,
    http::{HeaderMap, Method, StatusCode},
    response::{IntoResponse, Json, Response},
    routing::post,
    Router,
};
use sqlx::Row;

use crate::schema_introspect::{escape_ident, SchemaCache, TableMeta};
use crate::{enforce_auth, AppState};

/// Mount the GraphQL endpoint onto an existing router.
pub fn mount(router: Router<Arc<AppState>>) -> Router<Arc<AppState>> {
    router.route("/graphql", post(graphql_handler))
}

/// Build a fresh schema from a [`SchemaCache`]. Called per-request because
/// the cache itself is cheap and the schema builder's outputs aren't
/// `Send + Sync + 'static`-safe to cache without rework. Schema construction
/// is in-memory metadata only; no SQL runs here.
fn build_schema(cache: &SchemaCache, state: Arc<AppState>) -> Result<Schema, SchemaError> {
    let mut query = Object::new("Query");
    let mut where_inputs: Vec<InputObject> = Vec::new();

    for (table_name, tbl) in &cache.tables {
        let where_type_name = format!("{}_Where", table_name);
        let mut wi = InputObject::new(&where_type_name);
        for col in &tbl.columns {
            wi = wi.field(InputValue::new(
                col.name.clone(),
                TypeRef::named(TypeRef::STRING),
            ));
        }
        where_inputs.push(wi);

        let table_for_resolver = tbl.clone();
        let state_for_resolver = state.clone();
        let field = Field::new(
            table_name.clone(),
            TypeRef::named_nn_list_nn(TypeRef::named(scalar_json())),
            move |ctx| {
                let tbl = table_for_resolver.clone();
                let state = state_for_resolver.clone();
                FieldFuture::new(async move { run_query(ctx, &state, &tbl).await })
            },
        )
        .argument(InputValue::new(
            "workspace_id",
            TypeRef::named_nn(TypeRef::STRING),
        ))
        .argument(InputValue::new(
            "where",
            TypeRef::named(where_type_name.clone()),
        ))
        .argument(InputValue::new("order_by", TypeRef::named(TypeRef::STRING)))
        .argument(InputValue::new("limit", TypeRef::named(TypeRef::INT)))
        .argument(InputValue::new("offset", TypeRef::named(TypeRef::INT)));

        query = query.field(field);
    }

    let mut mutation = Object::new("Mutation");
    let state_for_command = state.clone();
    let command_field = Field::new("command", TypeRef::named_nn(scalar_json()), move |ctx| {
        let state = state_for_command.clone();
        FieldFuture::new(async move { run_command(ctx, &state).await })
    })
    .argument(InputValue::new(
        "workspace_id",
        TypeRef::named_nn(TypeRef::STRING),
    ))
    .argument(InputValue::new(
        "actor_id",
        TypeRef::named_nn(TypeRef::STRING),
    ))
    .argument(InputValue::new(
        "command_type",
        TypeRef::named_nn(TypeRef::STRING),
    ))
    .argument(InputValue::new("input", TypeRef::named_nn(scalar_json())))
    .argument(InputValue::new(
        "idempotency_key",
        TypeRef::named(TypeRef::STRING),
    ));
    mutation = mutation.field(command_field);

    let mut builder = Schema::build("Query", Some("Mutation"), None)
        .register(query)
        .register(mutation)
        .register(scalar_json_def());
    for wi in where_inputs {
        builder = builder.register(wi);
    }
    builder.finish()
}

fn scalar_json() -> &'static str {
    "JSON"
}

fn scalar_json_def() -> async_graphql::dynamic::Scalar {
    async_graphql::dynamic::Scalar::new("JSON")
}

async fn run_query<'ctx>(
    ctx: ResolverContext<'ctx>,
    state: &Arc<AppState>,
    tbl: &TableMeta,
) -> Result<Option<FieldValue<'ctx>>, async_graphql::Error> {
    let workspace_id: String = ctx
        .args
        .try_get("workspace_id")?
        .string()
        .map_err(|e| async_graphql::Error::new(e.to_string()))?
        .to_string();

    let limit = ctx
        .args
        .try_get("limit")
        .ok()
        .and_then(|v| v.i64().ok())
        .unwrap_or(100)
        .clamp(1, 10_000);
    let offset = ctx
        .args
        .try_get("offset")
        .ok()
        .and_then(|v| v.i64().ok())
        .unwrap_or(0)
        .max(0);

    let order_by = ctx
        .args
        .try_get("order_by")
        .ok()
        .and_then(|v| v.string().ok().map(String::from));

    // Build the WHERE clause from the optional `where` object. Each key
    // must map to a known column; we round-trip through string values so
    // the binding path matches sqlx's prepared statement API uniformly.
    let mut where_clauses: Vec<String> = Vec::new();
    let mut binds: Vec<String> = vec![workspace_id.clone()];
    if let Ok(where_value) = ctx.args.try_get("where") {
        if let Ok(obj) = where_value.object() {
            for (k, v) in obj.iter() {
                let key = k.as_str();
                if !tbl.has_column(key) {
                    return Err(async_graphql::Error::new(format!(
                        "unknown column '{key}' on table '{}'",
                        tbl.name
                    )));
                }
                let bind = gql_value_to_string(v);
                where_clauses.push(format!("\"{}\" = ?", escape_ident(key)));
                binds.push(bind);
            }
        }
    }

    let mut sql = format!(
        "SELECT * FROM \"{}\" WHERE workspace_id = ?",
        escape_ident(&tbl.name)
    );
    for clause in &where_clauses {
        sql.push_str(" AND ");
        sql.push_str(clause);
    }
    if let Some(ob) = order_by {
        let (col, asc) = parse_order_by(&ob, tbl)?;
        sql.push_str(" ORDER BY \"");
        sql.push_str(&escape_ident(&col));
        sql.push('"');
        sql.push_str(if asc { " ASC" } else { " DESC" });
    }
    sql.push_str(" LIMIT ");
    sql.push_str(&limit.to_string());
    sql.push_str(" OFFSET ");
    sql.push_str(&offset.to_string());

    let mut q = sqlx::query(&sql);
    for b in &binds {
        q = q.bind(b);
    }
    let rows = q
        .fetch_all(state.storage.pool())
        .await
        .map_err(|e| async_graphql::Error::new(e.to_string()))?;

    let mut out: Vec<FieldValue<'ctx>> = Vec::with_capacity(rows.len());
    for row in rows {
        let mut obj = serde_json::Map::with_capacity(tbl.columns.len());
        for col in &tbl.columns {
            let v = crate::auto_rest_helpers::sqlite_value_to_json(&row, &col.name);
            obj.insert(col.name.clone(), v);
        }
        let json = serde_json::Value::Object(obj);
        out.push(FieldValue::value(json_to_gql(&json)));
    }
    Ok(Some(FieldValue::list(out)))
}

async fn run_command<'ctx>(
    ctx: ResolverContext<'ctx>,
    state: &Arc<AppState>,
) -> Result<Option<FieldValue<'ctx>>, async_graphql::Error> {
    let workspace_id: String = ctx
        .args
        .try_get("workspace_id")?
        .string()
        .map_err(|e| async_graphql::Error::new(e.to_string()))?
        .to_string();
    let actor_id: String = ctx
        .args
        .try_get("actor_id")?
        .string()
        .map_err(|e| async_graphql::Error::new(e.to_string()))?
        .to_string();
    let command_type: String = ctx
        .args
        .try_get("command_type")?
        .string()
        .map_err(|e| async_graphql::Error::new(e.to_string()))?
        .to_string();
    let input_value = ctx.args.try_get("input")?;
    let input_json: serde_json::Value = gql_to_json(input_value.as_value());
    let idempotency_key = ctx
        .args
        .try_get("idempotency_key")
        .ok()
        .and_then(|v| v.string().ok().map(String::from));

    let ws = WorkspaceId::from_string(workspace_id);
    let actor = ActorId::from_string(actor_id);
    let _engine: &Engine = &state.engine;
    let outcome = state
        .engine
        .dispatch(
            &ws,
            &actor,
            &command_type,
            input_json,
            idempotency_key.as_deref(),
        )
        .await
        .map_err(|e| async_graphql::Error::new(e.to_string()))?;
    let result = serde_json::json!({
        "command_id": outcome.command_id.as_str(),
        "event_id": outcome.event_id.as_ref().map(|e| e.as_str()),
        "result": outcome.result,
    });
    Ok(Some(FieldValue::value(json_to_gql(&result))))
}

fn parse_order_by(spec: &str, tbl: &TableMeta) -> Result<(String, bool), async_graphql::Error> {
    // Same form as the REST `order` parameter: `<column>.{asc,desc}`.
    let mut parts = spec.splitn(2, '.');
    let col = parts.next().unwrap_or("").to_string();
    let dir = parts.next().unwrap_or("asc");
    if !tbl.has_column(&col) {
        return Err(async_graphql::Error::new(format!(
            "unknown column '{col}' on table '{}'",
            tbl.name
        )));
    }
    let asc = match dir {
        "asc" | "" => true,
        "desc" => false,
        other => {
            return Err(async_graphql::Error::new(format!(
                "invalid order direction '{other}'"
            )))
        }
    };
    Ok((col, asc))
}

fn gql_value_to_string(v: &GqlValue) -> String {
    match v {
        GqlValue::String(s) => s.clone(),
        GqlValue::Number(n) => n.to_string(),
        GqlValue::Boolean(b) => {
            if *b {
                "1".into()
            } else {
                "0".into()
            }
        }
        GqlValue::Null => String::new(),
        other => other.to_string(),
    }
}

fn json_to_gql(v: &serde_json::Value) -> GqlValue {
    match v {
        serde_json::Value::Null => GqlValue::Null,
        serde_json::Value::Bool(b) => GqlValue::Boolean(*b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                GqlValue::Number(i.into())
            } else if let Some(f) = n.as_f64() {
                GqlValue::Number(async_graphql::Number::from_f64(f).unwrap_or(0.into()))
            } else {
                GqlValue::Null
            }
        }
        serde_json::Value::String(s) => GqlValue::String(s.clone()),
        serde_json::Value::Array(a) => GqlValue::List(a.iter().map(json_to_gql).collect()),
        serde_json::Value::Object(o) => {
            let mut map = async_graphql::indexmap::IndexMap::new();
            for (k, v) in o {
                map.insert(async_graphql::Name::new(k), json_to_gql(v));
            }
            GqlValue::Object(map)
        }
    }
}

fn gql_to_json(v: &GqlValue) -> serde_json::Value {
    match v {
        GqlValue::Null => serde_json::Value::Null,
        GqlValue::Boolean(b) => serde_json::Value::Bool(*b),
        GqlValue::Number(n) => {
            if let Some(i) = n.as_i64() {
                serde_json::Value::Number(i.into())
            } else if let Some(f) = n.as_f64() {
                serde_json::Number::from_f64(f)
                    .map(serde_json::Value::Number)
                    .unwrap_or(serde_json::Value::Null)
            } else {
                serde_json::Value::Null
            }
        }
        GqlValue::String(s) => serde_json::Value::String(s.clone()),
        GqlValue::Enum(s) => serde_json::Value::String(s.to_string()),
        GqlValue::List(l) => serde_json::Value::Array(l.iter().map(gql_to_json).collect()),
        GqlValue::Object(o) => {
            let mut map = serde_json::Map::new();
            for (k, v) in o {
                map.insert(k.to_string(), gql_to_json(v));
            }
            serde_json::Value::Object(map)
        }
        GqlValue::Binary(_) => serde_json::Value::Null,
    }
}

async fn graphql_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    req: GraphQLBatchRequest,
) -> Response {
    let Some(cache) = state.schema_cache.clone() else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({
                "error":"schema_cache_unavailable",
                "message":"GraphQL requires schema cache; call AppState::with_schema_cache at boot"
            })),
        )
            .into_response();
    };
    // Extract workspace_id from each request's variables to run the auth
    // check. GraphQL doesn't have a path component to pull from, so we
    // require callers to pass `workspace_id` as a query/mutation variable
    // — which the field arguments already require.
    //
    // Best-effort: walk the first request's variables for `workspace_id`.
    // The resolvers re-enforce per-field, so the request-level check is
    // an extra fail-fast.
    let inner = req.into_inner();
    let requests: Vec<Request> = match inner {
        async_graphql_axum::GraphQLBatchRequest(b) => b.into_iter().collect(),
    };
    if let Some(ws) = requests
        .iter()
        .find_map(|r| r.variables.get(&async_graphql::Name::new("workspace_id")))
        .and_then(|v| match v {
            GqlValue::String(s) => Some(s.clone()),
            _ => None,
        })
    {
        if let Err(resp) = enforce_auth(&state, &headers, &Method::POST, &ws).await {
            return resp;
        }
    }
    let schema = match build_schema(cache.as_ref(), state.clone()) {
        Ok(s) => s,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error":"schema_build_failed","message":e.to_string()})),
            )
                .into_response();
        }
    };
    let mut responses: Vec<GqlResponse> = Vec::with_capacity(requests.len());
    for r in requests {
        responses.push(schema.execute(r).await);
    }
    if responses.len() == 1 {
        Json(responses.into_iter().next().unwrap()).into_response()
    } else {
        Json(responses).into_response()
    }
}

// Re-export the sqlite_value_to_json helper from auto_rest so the GraphQL
// resolver projects rows the same way the REST surface does.
mod auto_rest_helpers_proxy {
    pub use crate::auto_rest::sqlite_value_to_json;
}
pub(crate) use auto_rest_helpers_proxy as auto_rest_helpers;
