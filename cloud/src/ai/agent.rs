//! Client for accountir-agentd: routes chat turns to the company's persistent
//! Claude CLI session and persists the resulting messages for the chat UI.

use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

fn agentd_url() -> String {
    std::env::var("AGENTD_URL")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "http://127.0.0.1:9878".to_string())
}

/// Send one user turn to the company's agent session; store the user message
/// and every assistant message (text + tool_use blocks) in chat_messages.
pub async fn send_turn(
    pool: &PgPool,
    user_id: Uuid,
    company_id: Uuid,
    text: String,
) -> anyhow::Result<()> {
    crate::ai::chat::append_message(pool, user_id, company_id, "user", &json!(text))
        .await
        .map_err(|e| anyhow::anyhow!("store user msg: {e}"))?;

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(620))
        .build()?;
    let resp = client
        .post(format!("{}/turn", agentd_url()))
        .json(&json!({ "company_id": company_id, "user_id": user_id, "message": text }))
        .send()
        .await?;
    let body: Value = resp.json().await?;

    if !body["ok"].as_bool().unwrap_or(false) {
        let err = body["error"].as_str().unwrap_or("agent unavailable").to_string();
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

    let empty = vec![];
    for ev in body["events"].as_array().unwrap_or(&empty) {
        if ev["type"].as_str() == Some("assistant") {
            let content = ev["message"]["content"].clone();
            let has_blocks = content.as_array().map(|a| !a.is_empty()).unwrap_or(false);
            if has_blocks {
                crate::ai::chat::append_message(pool, user_id, company_id, "assistant", &content)
                    .await
                    .ok();
            }
        }
    }
    Ok(())
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
