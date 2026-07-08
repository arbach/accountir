//! Statement processor. Classifies each parsed statement transaction by running
//! ONE `claude` CLI turn over every source of evidence we hold — the address
//! book, prior classifications of similar transactions, and a live public
//! WebSearch for an unknown payee — then returns a structured verdict to post or
//! queue for human review. Write-side counterpart to the read-only
//! `accountir-recon` auditor; the classifier follows the same evidence-first
//! ladder the in-app agent uses.

use serde::Deserialize;
use serde_json::Value;
use tokio::io::AsyncWriteExt;

/// A parsed statement line awaiting classification.
#[derive(Debug, Clone)]
pub struct TxInput {
    /// YYYY-MM-DD.
    pub date: String,
    /// Cents; positive = money in, negative = money out (the bank's view).
    pub amount_cents: i64,
    pub memo: String,
    /// The bank/card/wallet account it posted to.
    pub account_hint: Option<String>,
}

/// Evidence gathered from our OWN data before the model is asked to decide.
#[derive(Debug, Clone, Default)]
pub struct Evidence {
    /// Address-book hits for any wallet/name in the memo ("name | kind | account_code").
    pub address_book: Vec<String>,
    /// Prior classifications of similar memos ("memo -> account_number (n times)").
    pub prior_tx: Vec<String>,
    /// The entity's chart, compact ("account_number name (type)").
    pub chart: Vec<String>,
}

/// The model's verdict for one transaction.
#[derive(Debug, Clone, Deserialize)]
pub struct Classification {
    /// Chart account number for the non-cash leg (e.g. "5300").
    pub account_number: String,
    /// Entity this really belongs to if mis-routed; empty = this entity.
    #[serde(default)]
    pub entity: String,
    /// Cleaned, human merchant/payee name.
    #[serde(default)]
    pub enriched_memo: String,
    /// 0.0–1.0. Below `AUTO_POST_THRESHOLD` → review queue, never auto-posted.
    pub confidence: f64,
    /// One line: which evidence drove the decision.
    #[serde(default)]
    pub reasoning: String,
}

/// Confidence at or above which a classification may be auto-posted.
pub const AUTO_POST_THRESHOLD: f64 = 0.75;

const CLASSIFIER_SYSTEM_PROMPT: &str = r#"You are a bookkeeping transaction classifier. Given ONE transaction and the evidence gathered from our own records, decide the single chart account its non-cash leg should post to.

EVIDENCE-FIRST — weigh the evidence in this order:
1. Address book: if a wallet/counterparty in the memo is labeled, its account_code IS the answer (lender->2490, income->revenue, contractor->5300, own->internal transfer).
2. Prior transactions: if a similar payee was booked before, book it the SAME way (consistency).
3. Public web search: if the payee is still unknown, WebSearch the merchant/company name to identify what it is. NEVER put amounts or account data in the query — search only the payee/merchant name.
4. Rules: Fidelity=NatlFinancial=CIBC; brokerage<->checking = Investments (no P&L); Wise = contractor (5300); card payments are transfers, not expenses; a transfer between the owner's own accounts is balance-sheet, never P&L; parse the FULL memo (e.g. "ORIG CO NAME:PROVIDERSCAREBIL" = "Providers Care Billing").

Respond with ONLY a JSON object, no prose, no code fences:
{"account_number":"<chart # for the non-cash leg>","entity":"<other entity name if mis-routed, else empty>","enriched_memo":"<clean payee name>","confidence":<0.0-1.0>,"reasoning":"<one line: which evidence decided it>"}
If the evidence is insufficient, return confidence below 0.5 so a human reviews it — never guess."#;

/// Classify one transaction via a single `claude` CLI turn (WebSearch enabled,
/// all file/shell tools disallowed).
pub async fn classify_tx(
    model: &str,
    tx: &TxInput,
    ev: &Evidence,
) -> anyhow::Result<Classification> {
    let prompt = format!(
        "TRANSACTION\n  date: {}\n  amount: {} ({})\n  memo: {}\n  posted to: {}\n\nEVIDENCE\n  address book:\n{}\n  prior similar tx:\n{}\n  chart of accounts:\n{}\n\nClassify it. JSON only.",
        tx.date,
        fmt_cents(tx.amount_cents),
        if tx.amount_cents >= 0 { "money in" } else { "money out" },
        tx.memo,
        tx.account_hint.as_deref().unwrap_or("?"),
        bullet(&ev.address_book),
        bullet(&ev.prior_tx),
        bullet(&ev.chart),
    );

    let mut cmd = tokio::process::Command::new("claude");
    cmd.arg("-p")
        .arg("--output-format").arg("json")
        .arg("--permission-mode").arg("dontAsk")
        .arg("--allowedTools").arg("WebSearch")
        .arg("--tools").arg("WebSearch")
        .arg("--disallowedTools")
        .args(["Bash", "Edit", "Write", "Read", "Glob", "Grep", "WebFetch", "Task", "Agent", "Skill"])
        .arg("--system-prompt").arg(CLASSIFIER_SYSTEM_PROMPT)
        .arg("--model").arg(model)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true);

    let mut child = cmd.spawn()?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(prompt.as_bytes()).await?;
        // drop closes stdin so claude treats the piped text as the prompt
    }
    let out = child.wait_with_output().await?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    // `claude -p --output-format json` prints an envelope whose `result` is the text.
    let text = match serde_json::from_str::<Value>(stdout.trim()) {
        Ok(env) => env
            .get("result")
            .and_then(|r| r.as_str())
            .map(str::to_string)
            .unwrap_or_else(|| stdout.to_string()),
        Err(_) => stdout.to_string(),
    };
    let json_str = extract_json_object(&text)
        .ok_or_else(|| anyhow::anyhow!("classifier returned no JSON object"))?;
    Ok(serde_json::from_str(&json_str)?)
}

fn fmt_cents(c: i64) -> String {
    format!("${:.2}", c as f64 / 100.0)
}

fn bullet(v: &[String]) -> String {
    if v.is_empty() {
        "    (none)".to_string()
    } else {
        v.iter().map(|s| format!("    - {s}")).collect::<Vec<_>>().join("\n")
    }
}

/// Pull the first balanced `{...}` object out of possibly-fenced model text.
fn extract_json_object(s: &str) -> Option<String> {
    let start = s.find('{')?;
    let end = s.rfind('}')?;
    (end > start).then(|| s[start..=end].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_json_from_fenced_text() {
        let t = "Sure!\n```json\n{\"account_number\":\"5300\",\"confidence\":0.9}\n```\n";
        let j = extract_json_object(t).unwrap();
        let c: Classification = serde_json::from_str(&j).unwrap();
        assert_eq!(c.account_number, "5300");
        assert!(c.confidence > AUTO_POST_THRESHOLD);
        assert!(c.entity.is_empty());
    }
}
