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

/// Resolve the bearer token to (company_id, acting user_id, is_personal).
async fn auth_company(state: &AppState, headers: &HeaderMap) -> Option<(Uuid, Uuid, bool)> {
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
    let (owner, is_personal): (Uuid, bool) =
        sqlx::query_as("SELECT owner_user_id, is_personal FROM companies WHERE id = $1")
            .bind(company_id)
            .fetch_optional(&state.pool)
            .await
            .ok()
            .flatten()?;
    let user_id = last_user.unwrap_or(owner);
    Some((company_id, user_id, is_personal))
}

/// For personal-entity sessions: resolve an `entity` argument (uuid, name, or
/// slug) to a company the acting user is a member of. Membership is the gate —
/// an unknown or unauthorized entity returns None.
async fn resolve_entity(
    state: &AppState,
    user_id: Uuid,
    entity: &str,
) -> Option<(Uuid, String)> {
    let companies = crate::queries::list_companies_for_user(&state.pool, user_id)
        .await
        .ok()?;
    let wanted = entity.trim().to_lowercase();
    companies
        .into_iter()
        .find(|c| {
            c.id.to_string() == wanted
                || c.name.to_lowercase() == wanted
                || c.slug.to_lowercase() == wanted
        })
        .map(|c| (c.id, c.name))
}

/// Tool schemas in MCP shape (`inputSchema`, not Anthropic's `input_schema`).
/// Personal-entity sessions additionally get a `list_entities` tool, and every
/// tool gains an optional `entity` parameter for cross-entity operation.
fn mcp_tool_list(is_personal: bool) -> Vec<Value> {
    let mut out: Vec<Value> = tools::schemas()
        .into_iter()
        .map(|mut t| {
            if let Some(obj) = t.as_object_mut() {
                if let Some(mut schema) = obj.remove("input_schema") {
                    if is_personal {
                        if let Some(props) =
                            schema.get_mut("properties").and_then(|p| p.as_object_mut())
                        {
                            props.insert(
                                "entity".to_string(),
                                json!({
                                    "type": "string",
                                    "description": "Optional: operate on another of the user's entities (name, slug, or id from list_entities). Defaults to this personal entity."
                                }),
                            );
                        }
                    }
                    obj.insert("inputSchema".to_string(), schema);
                }
            }
            t
        })
        .collect();
    if is_personal {
        out.push(json!({
            "name": "list_entities",
            "description": "List every entity (company) this user can manage, including this personal one. Use the returned name as the `entity` parameter on any other tool to operate on that entity.",
            "inputSchema": { "type": "object", "properties": {} }
        }));
        out.push(json!({
            "name": "move_file",
            "description": "Re-file a document that was uploaded into this personal session so it is stored under the entity it actually belongs to. Use the file_id from the upload manifest. The original bytes are moved to the target entity's file store (and de-duplicated). After moving, post the document's accounting under that same entity using the `entity` parameter.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file_id": { "type": "string", "description": "The file id from the upload manifest." },
                    "to_entity": { "type": "string", "description": "Target entity name, slug, or id (from list_entities)." }
                },
                "required": ["file_id", "to_entity"]
            }
        }));
    }
    out
}

async fn mcp_post(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(rpc): Json<Value>,
) -> Response {
    let Some((company_id, user_id, is_personal)) = auth_company(&state, &headers).await else {
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
        "tools/list" => rpc_result(&id, json!({ "tools": mcp_tool_list(is_personal) })),
        "tools/call" => {
            let name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let mut args = params.get("arguments").cloned().unwrap_or(json!({}));

            // The session's own company, before any per-call entity re-scoping —
            // move_file needs it as the source the document was uploaded into.
            let session_company = company_id;
            // Personal-entity sessions may re-scope a call to another entity
            // the user is a member of via the optional `entity` argument.
            let mut company_id = company_id;
            let entity_arg = args
                .as_object_mut()
                .and_then(|o| o.remove("entity"))
                .and_then(|v| v.as_str().map(str::to_string))
                .filter(|s| !s.trim().is_empty());
            if let Some(entity) = entity_arg {
                if !is_personal {
                    return rpc_result(
                        &id,
                        json!({
                            "content": [{ "type": "text", "text": json!({
                                "error": "cross-entity access is only available from the personal entity's session"
                            }).to_string() }],
                            "isError": true
                        }),
                    );
                }
                match resolve_entity(&state, user_id, &entity).await {
                    Some((target_id, target_name)) => {
                        tracing::info!(from = %company_id, to = %target_id, entity = %target_name,
                            "personal session cross-entity call");
                        company_id = target_id;
                    }
                    None => {
                        return rpc_result(
                            &id,
                            json!({
                                "content": [{ "type": "text", "text": json!({
                                    "error": format!("unknown entity '{entity}' or no access — use list_entities")
                                }).to_string() }],
                                "isError": true
                            }),
                        );
                    }
                }
            }
            tracing::info!(company = %company_id, tool = name, "agent tool call");

            // sync_bank needs full AppState (Plaid config), so it can't live in
            // ai::tools::execute which only sees the pool.
            let out: Value = if name == "list_entities" {
                if !is_personal {
                    json!({ "error": "list_entities is only available from the personal entity's session" })
                } else {
                    match crate::queries::list_companies_for_user(&state.pool, user_id).await {
                        Ok(companies) => json!({
                            "entities": companies.iter().map(|c| json!({
                                "id": c.id, "name": c.name, "slug": c.slug, "role": c.role,
                            })).collect::<Vec<_>>()
                        }),
                        Err(e) => json!({ "error": format!("{e}") }),
                    }
                }
            } else if name == "move_file" {
                if !is_personal {
                    json!({ "error": "move_file is only available from the personal entity's session" })
                } else {
                    let file_id = args
                        .get("file_id")
                        .and_then(|v| v.as_str())
                        .and_then(|s| Uuid::parse_str(s.trim()).ok());
                    let to_entity = args
                        .get("to_entity")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    match (file_id, to_entity.trim().is_empty()) {
                        (None, _) => json!({ "error": "file_id must be a valid file id from the upload manifest" }),
                        (_, true) => json!({ "error": "to_entity is required" }),
                        (Some(fid), false) => match resolve_entity(&state, user_id, &to_entity).await {
                            None => json!({ "error": format!("unknown entity '{to_entity}' or no access — use list_entities") }),
                            Some((target_id, target_name)) => {
                                match crate::file_store::move_company_file(
                                    &state.pool, session_company, target_id, fid,
                                ).await {
                                    Ok(new_id) => {
                                        tracing::info!(from = %session_company, to = %target_id,
                                            file = %fid, "personal session moved file to entity");
                                        json!({ "ok": true, "file_id": new_id, "entity": target_name })
                                    }
                                    Err(e) => json!({ "error": e }),
                                }
                            }
                        },
                    }
                }
            } else if name == "sync_bank" {
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
