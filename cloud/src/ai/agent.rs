//! Client for accountir-agentd: routes chat turns to the company's persistent
//! Claude CLI session, live-forwards stream events, and persists the resulting
//! messages for the chat UI.

use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

pub fn agentd_url() -> String {
    std::env::var("AGENTD_URL")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "http://127.0.0.1:9878".to_string())
}

/// Send one user turn to the company's agent session. Every stream-json event
/// is forwarded into `forward` as it arrives (best-effort — a slow or absent
/// listener never blocks the turn). On completion the user message and all
/// assistant messages are persisted to chat_messages.
pub async fn stream_turn(
    pool: &PgPool,
    user_id: Uuid,
    company_id: Uuid,
    text: String,
    forward: tokio::sync::mpsc::Sender<Value>,
) -> anyhow::Result<()> {
    crate::ai::chat::append_message(pool, user_id, company_id, "user", &json!(text))
        .await
        .map_err(|e| anyhow::anyhow!("store user msg: {e}"))?;

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(620))
        .build()?;
    let mut resp = client
        .post(format!("{}/turn", agentd_url()))
        .json(&json!({ "company_id": company_id, "user_id": user_id, "message": text }))
        .send()
        .await?
        .error_for_status()?;

    let mut buf: Vec<u8> = Vec::new();
    let mut assistant_msgs: Vec<Value> = Vec::new();
    let mut daemon_err: Option<String> = None;

    while let Some(chunk) = resp.chunk().await? {
        buf.extend_from_slice(&chunk);
        while let Some(pos) = buf.iter().position(|b| *b == b'\n') {
            let line: Vec<u8> = buf.drain(..=pos).collect();
            let Ok(v) = serde_json::from_slice::<Value>(&line) else {
                continue;
            };
            match v["type"].as_str() {
                Some("assistant") => {
                    let content = v["message"]["content"].clone();
                    if content.as_array().map(|a| !a.is_empty()).unwrap_or(false) {
                        assistant_msgs.push(content);
                    }
                }
                Some("daemon_error") => {
                    daemon_err =
                        Some(v["error"].as_str().unwrap_or("agent error").to_string());
                }
                _ => {}
            }
            let _ = forward.try_send(v);
        }
    }

    for content in &assistant_msgs {
        crate::ai::chat::append_message(pool, user_id, company_id, "assistant", content)
            .await
            .ok();
    }
    if let Some(err) = daemon_err {
        crate::ai::chat::append_message(
            pool,
            user_id,
            company_id,
            "assistant",
            &json!(format!("(agent error: {err})")),
        )
        .await
        .ok();
        anyhow::bail!("agent turn failed: {err}");
    }
    Ok(())
}

/// Blocking variant for the no-JS form fallback: runs the turn to completion,
/// discarding live events (messages are still persisted).
pub async fn send_turn(
    pool: &PgPool,
    user_id: Uuid,
    company_id: Uuid,
    text: String,
) -> anyhow::Result<()> {
    let (tx, _rx) = tokio::sync::mpsc::channel(8);
    stream_turn(pool, user_id, company_id, text, tx).await
}

/// Forget the company's agent session (kills the live process and rotates the
/// session id on next use). Best-effort.
pub async fn reset_session(company_id: Uuid) {
    let client = reqwest::Client::new();
    let _ = client
        .post(format!("{}/reset", agentd_url()))
        .json(&json!({ "company_id": company_id }))
        .send()
        .await;
}
