//! Chat history persistence. Turns themselves run on the per-company Claude
//! CLI session via accountir-agentd (see ai::agent); this module only stores
//! and loads the conversation for the UI.

use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum ChatError {
    #[error("db: {0}")]
    Db(#[from] sqlx::Error),
}

#[derive(Debug, Clone)]
pub struct StoredMessage {
    pub role: String,
    pub content: Value,
}

pub async fn load_history(
    pool: &PgPool,
    user_id: Uuid,
    company_id: Uuid,
) -> Result<Vec<StoredMessage>, ChatError> {
    let rows = sqlx::query_as::<_, (String, Value)>(
        "SELECT role, content FROM chat_messages WHERE user_id = $1 AND company_id = $2 ORDER BY created_at ASC, id ASC",
    )
    .bind(user_id)
    .bind(company_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(role, content)| StoredMessage { role, content })
        .collect())
}

pub async fn append_message(
    pool: &PgPool,
    user_id: Uuid,
    company_id: Uuid,
    role: &str,
    content: &Value,
) -> Result<(), ChatError> {
    sqlx::query(
        "INSERT INTO chat_messages (user_id, company_id, role, content) VALUES ($1, $2, $3, $4)",
    )
    .bind(user_id)
    .bind(company_id)
    .bind(role)
    .bind(content)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn clear_history(pool: &PgPool, user_id: Uuid, company_id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM chat_messages WHERE user_id = $1 AND company_id = $2")
        .bind(user_id)
        .bind(company_id)
        .execute(pool)
        .await?;
    Ok(())
}
