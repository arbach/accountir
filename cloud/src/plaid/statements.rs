//! Plaid Statements parsing: take a downloaded statement PDF, extract its text,
//! and use Claude to turn it into structured transaction lines.
//!
//! Bank statement PDFs have no standard layout, so heuristic parsing is brittle.
//! We lean on the LLM (the same Anthropic client the chat feature uses) to extract
//! a normalized list of transactions, which are then staged for user review.

use serde::Deserialize;
use serde_json::json;

use crate::ai::anthropic::{AnthropicClient, Message};

/// One parsed transaction line from a statement.
#[derive(Debug, Clone, Deserialize)]
pub struct ParsedLine {
    /// ISO date `YYYY-MM-DD`.
    pub date: String,
    pub description: String,
    /// Signed amount in cents: negative = money out, positive = money in.
    pub amount_cents: i64,
}

/// Extract plain text from a statement PDF. `pdf-extract` can panic on malformed
/// input, so we isolate it with `catch_unwind`.
pub fn extract_text(pdf: &[u8]) -> Result<String, String> {
    let bytes = pdf.to_vec();
    match std::panic::catch_unwind(move || pdf_extract::extract_text_from_mem(&bytes)) {
        Ok(Ok(text)) => Ok(text),
        Ok(Err(e)) => Err(format!("pdf extract error: {e}")),
        Err(_) => Err("pdf extract panicked on this file".to_string()),
    }
}

const SYSTEM: &str = "You are a precise bank/credit-card statement parser. \
From the statement text, extract EVERY individual transaction line. \
Respond with ONLY a JSON array — no prose, no markdown code fences. \
Each element must be an object: {\"date\":\"YYYY-MM-DD\",\"description\":string,\"amount_cents\":integer}. \
`amount_cents` is the signed amount in integer cents: NEGATIVE for money leaving the account \
(purchases, withdrawals, debits, payments out) and POSITIVE for money coming in (deposits, refunds, credits). \
Infer the full year from the statement period/closing date. \
Do NOT include running balances, summaries, totals, interest-rate lines, or marketing text — only real transactions. \
If there are no transactions, respond with [].";

/// Parse statement text into transaction lines using Claude.
pub async fn parse_with_ai(api_key: &str, statement_text: &str) -> Result<Vec<ParsedLine>, String> {
    // Bound input to keep the request within sane size/cost limits.
    let text: String = statement_text.chars().take(60_000).collect();
    let client = AnthropicClient::new(api_key.to_string());
    let messages = vec![Message {
        role: "user".to_string(),
        content: json!(format!("Statement text:\n\n{text}")),
    }];
    let resp = client
        .create_message(SYSTEM, &messages, None)
        .await
        .map_err(|e| format!("anthropic error: {e}"))?;

    let mut out = String::new();
    for block in &resp.content {
        if block.get("type").and_then(|t| t.as_str()) == Some("text") {
            if let Some(t) = block.get("text").and_then(|t| t.as_str()) {
                out.push_str(t);
            }
        }
    }

    let json_str = extract_json_array(&out);
    serde_json::from_str::<Vec<ParsedLine>>(&json_str).map_err(|e| {
        let preview: String = out.chars().take(200).collect();
        format!("could not parse AI response as transactions: {e}; got: {preview}")
    })
}

/// Pull the outermost JSON array out of an LLM response, tolerating stray prose
/// or markdown fences.
fn extract_json_array(s: &str) -> String {
    let t = s.trim();
    let t = t
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();
    if let (Some(start), Some(end)) = (t.find('['), t.rfind(']')) {
        if end > start {
            return t[start..=end].to_string();
        }
    }
    t.to_string()
}
