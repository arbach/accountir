//! MCP (Model Context Protocol) endpoint for the per-company Claude agent.
//!
//! Speaks JSON-RPC 2.0 over streamable HTTP (single JSON responses). The agent
//! daemon points each company's `claude` process here with a per-company bearer
//! token; the token resolves to exactly one company, and every tool call runs
//! company-scoped through the same code paths as the web UI.

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::ai::tools::{self, ToolContext};
use crate::http::AppState;

pub fn router() -> Router<AppState> {
    Router::new().route("/mcp", post(mcp_post))
}

fn rpc_result(id: &Value, result: Value) -> Response {
    Json(json!({ "jsonrpc": "2.0", "id": id, "result": result })).into_response()
}

fn rpc_error(id: &Value, code: i64, message: &str) -> Response {
    Json(json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } }))
        .into_response()
}

/// Resolve the bearer token to (company_id, acting user_id).
async fn auth_company(state: &AppState, headers: &HeaderMap) -> Option<(Uuid, Uuid)> {
    let token = headers
        .get("authorization")?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")?
        .trim()
        .to_string();
    let row: Option<(Uuid, Option<Uuid>)> = sqlx::query_as(
        "SELECT company_id, last_user_id FROM agent_sessions WHERE mcp_token = $1",
    )
    .bind(&token)
    .fetch_optional(&state.pool)
    .await
    .ok()
    .flatten();
    let (company_id, last_user) = row?;
    let user_id = match last_user {
        Some(u) => u,
        None => {
            // Fall back to the company owner for event attribution.
            sqlx::query_as::<_, (Uuid,)>("SELECT owner_user_id FROM companies WHERE id = $1")
                .bind(company_id)
                .fetch_optional(&state.pool)
                .await
                .ok()
                .flatten()?
                .0
        }
    };
    Some((company_id, user_id))
}

/// Tool schemas in MCP shape (`inputSchema`, not Anthropic's `input_schema`).
fn mcp_tool_list() -> Vec<Value> {
    tools::schemas()
        .into_iter()
        .map(|mut t| {
            if let Some(obj) = t.as_object_mut() {
                if let Some(schema) = obj.remove("input_schema") {
                    obj.insert("inputSchema".to_string(), schema);
                }
            }
            t
        })
        .collect()
}

async fn mcp_post(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(rpc): Json<Value>,
) -> Response {
    let Some((company_id, user_id)) = auth_company(&state, &headers).await else {
        return (StatusCode::UNAUTHORIZED, "invalid bearer token").into_response();
    };

    let id = rpc.get("id").cloned().unwrap_or(Value::Null);
    let method = rpc.get("method").and_then(|m| m.as_str()).unwrap_or("");
    let params = rpc.get("params").cloned().unwrap_or(json!({}));

    match method {
        "initialize" => {
            let proto = params
                .get("protocolVersion")
                .and_then(|v| v.as_str())
                .unwrap_or("2025-06-18");
            rpc_result(
                &id,
                json!({
                    "protocolVersion": proto,
                    "capabilities": { "tools": {} },
                    "serverInfo": { "name": "accountir-accounting", "version": "1.0.0" }
                }),
            )
        }
        // Notifications carry no id and expect no JSON-RPC response.
        m if m.starts_with("notifications/") => StatusCode::ACCEPTED.into_response(),
        "ping" => rpc_result(&id, json!({})),
        "tools/list" => rpc_result(&id, json!({ "tools": mcp_tool_list() })),
        "tools/call" => {
            let name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let args = params.get("arguments").cloned().unwrap_or(json!({}));
            tracing::info!(company = %company_id, tool = name, "agent tool call");

            // sync_bank needs full AppState (Plaid config), so it can't live in
            // ai::tools::execute which only sees the pool.
            let out: Value = if name == "sync_bank" {
                match args
                    .get("item_id")
                    .and_then(|v| v.as_str())
                    .and_then(|s| Uuid::parse_str(s).ok())
                {
                    Some(item) => {
                        match crate::plaid::sync::run_sync_for_item(&state, company_id, user_id, item)
                            .await
                        {
                            Ok((imported, skipped)) => {
                                json!({ "ok": true, "imported": imported, "skipped": skipped })
                            }
                            Err(e) => json!({ "error": format!("sync failed: {e:?}") }),
                        }
                    }
                    None => json!({ "error": "item_id must be a UUID" }),
                }
            } else {
                let ctx = ToolContext { pool: &state.pool, company_id, user_id };
                tools::execute(name, &args, &ctx).await
            };

            let is_error = out.get("error").is_some();
            rpc_result(
                &id,
                json!({
                    "content": [{ "type": "text", "text": out.to_string() }],
                    "isError": is_error
                }),
            )
        }
        other => rpc_error(&id, -32601, &format!("method not found: {other}")),
    }
}
