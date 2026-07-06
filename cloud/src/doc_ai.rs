//! Reconcile a supporting document against the transaction it was attached to.
//!
//! When a document (receipt, invoice, contract, …) is added to a journal entry,
//! we extract its text, ask the agent daemon to analyse it against the entry,
//! and apply safe updates: classify the document, write a summary note, set the
//! category when missing, and improve a vague memo (only when the document
//! clearly matches the transaction — the memo edit is event-sourced/reversible).

use crate::commands::mutations;
use crate::queries;
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

const DOC_TYPES: &[&str] = &["receipt", "invoice", "contract", "statement", "tax", "other"];

const SYSTEM: &str = "You are a bookkeeping assistant. You are given the TEXT of a supporting \
document that a user attached to a specific accounting transaction, plus that transaction's \
current details. Analyse the document and reconcile it against the transaction. \
Reply with ONLY a JSON object (no prose, no code fences) with exactly these keys: \
\"doc_type\" (one of: receipt, invoice, contract, statement, tax, other), \
\"vendor\" (the merchant/counterparty named on the document, or null), \
\"category\" (a short spend/category tag such as \"Meals\", \"Software\", \"Travel\", \"Loan\", \
\"Rent\", \"Utilities\", or null if unclear), \
\"document_total\" (the total amount shown on the document as a string, or null), \
\"matches_transaction\" (true only if the document plainly corresponds to this transaction by \
amount, date, and/or vendor), \
\"improved_memo\" (a concise, specific description for the transaction — vendor + what was \
bought — ONLY if the current memo is vague or generic and the document clearly matches; \
otherwise null), \
\"summary\" (one or two plain sentences: what the document is, its total/date/vendor, and \
whether it matches the transaction). Never invent figures the document does not show.";

/// Analyse one attached document and update the entry. Best-effort: any failure
/// is recorded as the document's note so the user sees what happened.
pub async fn process_entry_document(
    pool: PgPool,
    company_id: Uuid,
    user_id: Uuid,
    entry_id: Uuid,
    file_id: Uuid,
    filename: String,
    bytes: Vec<u8>,
) {
    let note = |t: &str, n: &str| {
        let pool = pool.clone();
        let n = n.to_string();
        let t = t.to_string();
        async move {
            let _ = queries::annotate_entry_document(&pool, company_id, entry_id, file_id, &t, &n).await;
        }
    };

    // 1. Extract the document text.
    let text = match extract_text(&bytes).await {
        Ok(t) if !t.trim().is_empty() => t,
        Ok(_) => return note("other", "No readable text could be extracted from this document.").await,
        Err(e) => return note("other", &format!("Could not read the document: {e}")).await,
    };

    // 2. Transaction context.
    let ctx = match queries::entry_context_text(&pool, company_id, entry_id).await {
        Ok(Some(c)) => c,
        _ => return,
    };
    let had_category = ctx.contains("Current category:");

    // 3. Ask the agent daemon.
    let doc: String = text.chars().take(30_000).collect();
    let prompt = format!(
        "TRANSACTION:\n{ctx}\n\nDOCUMENT (\"{filename}\"):\n{doc}\n\nReturn the JSON object now."
    );
    let analysis = match oneshot_json(SYSTEM, &prompt).await {
        Ok(v) => v,
        Err(e) => return note("other", &format!("Document stored; automatic analysis failed: {e}")).await,
    };

    // 4. Apply.
    let doc_type = analysis["doc_type"].as_str().unwrap_or("other");
    let doc_type = if DOC_TYPES.contains(&doc_type) { doc_type } else { "other" };
    let matches = analysis["matches_transaction"].as_bool().unwrap_or(false);
    let summary = analysis["summary"].as_str().unwrap_or("").trim();
    let mut note_text = if summary.is_empty() {
        "Document analysed.".to_string()
    } else {
        summary.to_string()
    };
    if !matches {
        note_text.push_str("  ⚠ This document may not match this transaction — please review.");
    }

    // Category: set only when the entry has none yet.
    if let Some(cat) = analysis["category"].as_str().map(str::trim).filter(|s| !s.is_empty()) {
        if !had_category {
            let _ = queries::set_entry_category(&pool, company_id, entry_id, cat).await;
        }
    }

    // Memo: improve it only when the document matches the transaction.
    if matches {
        if let Some(m) = analysis["improved_memo"].as_str().map(str::trim).filter(|s| !s.is_empty()) {
            if m.len() <= 200 {
                let _ = mutations::update_entry_memo(&pool, company_id, user_id, entry_id, m).await;
            }
        }
    }

    note(doc_type, &note_text).await;
}

/// PDF -> text/OCR; raster image -> OCR; otherwise treat as UTF-8 text.
async fn extract_text(bytes: &[u8]) -> Result<String, String> {
    if bytes.starts_with(b"%PDF") {
        crate::plaid::statements::extract_text_or_ocr(bytes).await
    } else if bytes.starts_with(b"\x89PNG")
        || bytes.starts_with(&[0xFF, 0xD8, 0xFF])
        || bytes.starts_with(b"GIF8")
    {
        crate::plaid::statements::ocr_image(bytes).await
    } else if bytes.contains(&0) {
        Err("unsupported binary format".into())
    } else {
        Ok(String::from_utf8_lossy(bytes).to_string())
    }
}

/// Call the agent daemon's stateless /oneshot endpoint and parse a JSON object.
async fn oneshot_json(system: &str, prompt: &str) -> Result<Value, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .map_err(|e| format!("http client: {e}"))?;
    let resp = client
        .post(format!("{}/oneshot", crate::ai::agent::agentd_url()))
        .json(&json!({ "system": system, "prompt": prompt }))
        .send()
        .await
        .map_err(|e| format!("agent daemon unreachable: {e}"))?;
    let body: Value = resp.json().await.map_err(|e| format!("bad daemon reply: {e}"))?;
    if !body["ok"].as_bool().unwrap_or(false) {
        return Err(body["error"].as_str().unwrap_or("unknown").to_string());
    }
    let out = body["result"].as_str().unwrap_or("");
    let t = out
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();
    let json_str = match (t.find('{'), t.rfind('}')) {
        (Some(s), Some(e)) if e > s => &t[s..=e],
        _ => t,
    };
    serde_json::from_str::<Value>(json_str).map_err(|e| format!("bad JSON from agent: {e}"))
}
