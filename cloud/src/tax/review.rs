//! Step-4 form review — replaces the old one-shot AI verifier (`verify.rs`).
//!
//! Design: **deterministic coverage, AI correctness**.
//!   1. Deterministically decide which forms an entity must file (`applicable_forms`).
//!   2. Deterministically enumerate EVERY item on each form — every filled field,
//!      every expected computed line, every checkbox (`enumerate_items`). This is
//!      what guarantees nothing is skipped; the old verifier chose what to look at
//!      and drifted.
//!   3. For each item, one FOCUSED AI check judges correctness against the booking
//!      + profile (`check_item`). One item per call → the model can't skip or
//!      hallucinate across a whole form.
//!   4. Aggregate. A form is ready only when every enumerated item passes.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use tokio::io::AsyncWriteExt;

// ─────────────────────────── 1. Deterministic: applicability ───────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct EntityFacts {
    pub kind: String, // individual | s_corp | c_corp
    pub state: String,
    pub receipts: i64,
    pub total_assets: i64,
    pub rental: bool,
    pub shareholders: u32,
    pub files_8832: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct RequiredForm {
    pub code: String,
    pub required: bool, // false = explicitly NOT needed (exempt), kept for transparency
    pub reason: String,
}

const SCH_L_M1_THRESHOLD: i64 = 250_000;

/// Which forms this entity must file (and which are explicitly exempt), with reasons.
pub fn applicable_forms(f: &EntityFacts) -> Vec<RequiredForm> {
    let mut v = Vec::new();
    let need = |code: &str, reason: &str| RequiredForm { code: code.into(), required: true, reason: reason.into() };
    let skip = |code: &str, reason: &str| RequiredForm { code: code.into(), required: false, reason: reason.into() };

    match f.kind.as_str() {
        "individual" => {
            v.push(need("f1040", "individual income tax return"));
            if f.state == "IL" { v.push(need("il1040", "Illinois resident return")); }
        }
        "s_corp" => {
            v.push(need("f1120s", "S-corp return"));
            v.push(need("f1120ssk", "Schedule K-1 (one per shareholder)"));
            if f.rental { v.push(need("f8825", "rental real estate activity")); }
            sched_l_m1(f, &mut v, "1120-S");
            if f.state == "IL" { v.push(need("il1120st", "Illinois small-business replacement tax")); }
        }
        "c_corp" => {
            v.push(need("f1120", "C-corp return"));
            sched_l_m1(f, &mut v, "1120");
            if f.state == "IL" { v.push(need("il1120", "Illinois corporate income + replacement tax")); }
        }
        _ => {}
    }
    if f.files_8832 { v.push(need("f8832", "entity classification election (elective)")); }
    v
}

fn sched_l_m1(f: &EntityFacts, v: &mut Vec<RequiredForm>, form: &str) {
    if f.receipts < SCH_L_M1_THRESHOLD && f.total_assets < SCH_L_M1_THRESHOLD {
        v.push(RequiredForm { code: format!("{form}-SchL"), required: false,
            reason: format!("receipts ${} & assets ${} both < $250k", f.receipts, f.total_assets) });
        v.push(RequiredForm { code: format!("{form}-SchM1"), required: false, reason: "same exemption".into() });
    } else {
        v.push(RequiredForm { code: format!("{form}-SchL"), required: true,
            reason: format!("receipts ${} or assets ${} >= $250k", f.receipts, f.total_assets) });
        v.push(RequiredForm { code: format!("{form}-SchM1"), required: true, reason: "book-to-tax reconciliation".into() });
    }
}

// ─────────────────────────── 2. Deterministic: item enumeration ───────────────────────────

/// One thing on a form that must be verified. `field` is the AcroForm field it lives in
/// (None => the expected value is not yet placed in any field — an automatic failure the
/// AI still explains).
#[derive(Debug, Clone, Serialize)]
pub struct FormItem {
    pub line: String,
    pub field: Option<String>,
    pub on_form: String,       // the actual value/checkbox state read off the form
    pub expected: String,      // expected value from the computed return / profile
    pub kind: String,          // amount | text | checkbox
}

/// Read the filled PDF's field values + checked boxes via the single Deno helper.
pub fn field_state(pdf_path: &str) -> (HashMap<String, String>, HashSet<String>) {
    let helper = std::env::var("TAXPDF_BIN")
        .unwrap_or_else(|_| "/usr/local/lib/accountir/tax/taxpdf.ts".to_string());
    let out = std::process::Command::new("deno")
        .args(["run", "--allow-read", &helper, "dump", pdf_path])
        .output();
    let mut vals = HashMap::new();
    let mut checked = HashSet::new();
    if let Ok(o) = out {
        for ln in String::from_utf8_lossy(&o.stdout).lines() {
            if let Some(rest) = ln.strip_prefix("T\t") {
                if let Some((n, v)) = rest.split_once('\t') { vals.insert(n.to_string(), v.to_string()); }
            } else if let Some(n) = ln.strip_prefix("C\t") {
                checked.insert(n.to_string());
            }
        }
    }
    (vals, checked)
}

/// Deterministically build the full item list for one form: pair every EXPECTED line
/// (from the computed return + profile mapping) with what is actually on the form.
/// `expected` is the per-form spec: [{line, field, expected, kind}].
pub fn enumerate_items(pdf_path: &str, expected: &[Value]) -> Vec<FormItem> {
    let (vals, checked) = field_state(pdf_path);
    expected.iter().map(|e| {
        let field = e.get("field").and_then(|f| f.as_str()).map(str::to_string);
        let kind = e.get("kind").and_then(|k| k.as_str()).unwrap_or("amount").to_string();
        let on_form = match kind.as_str() {
            "checkbox" => field.as_ref().map(|f| if checked.contains(f) { "checked" } else { "UNCHECKED" }.to_string())
                .unwrap_or_else(|| "UNCHECKED".into()),
            _ => field.as_ref().and_then(|f| vals.get(f)).cloned().unwrap_or_else(|| "(blank/not in field)".into()),
        };
        FormItem {
            line: e.get("line").and_then(|l| l.as_str()).unwrap_or("").to_string(),
            field, on_form,
            expected: e.get("expected").map(value_to_string).unwrap_or_default(),
            kind,
        }
    }).collect()
}

fn value_to_string(v: &Value) -> String {
    match v { Value::String(s) => s.clone(), other => other.to_string() }
}

// ─────────────────────────── 3. AI: per-item correctness ───────────────────────────

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ItemVerdict {
    pub line: String,
    pub ok: bool,
    #[serde(default)]
    pub note: String,
}

const ITEM_PROMPT: &str = "You verify ONE line of a filled US tax form. You are given the line label, the value currently on the form, and the authoritative expected value (from the taxpayer's booking/profile). Decide only whether the form's value correctly matches the expected value for that line (right amount/sign, or right identity text, or the checkbox in the correct state). A blank signature is fine. Respond with ONLY JSON: {\"ok\":<bool>,\"note\":\"<empty if ok, else the problem>\"}.";

/// One focused AI call per item — the deterministic loop guarantees every item is
/// checked; this only judges correctness of the single item it is handed.
pub async fn check_item(model: &str, item: &FormItem) -> anyhow::Result<ItemVerdict> {
    let prompt = format!(
        "LINE: {}\nKIND: {}\nON FORM: {}\nEXPECTED: {}\nJSON only.",
        item.line, item.kind, item.on_form, item.expected
    );
    let mut cmd = tokio::process::Command::new("claude");
    cmd.arg("-p")
        .arg("--output-format").arg("json")
        .arg("--permission-mode").arg("dontAsk")
        .arg("--allowedTools").arg("")
        .arg("--disallowedTools").args(["Bash", "Edit", "Write", "Read", "Glob", "Grep", "WebFetch", "WebSearch", "Task", "Agent", "Skill"])
        .arg("--system-prompt").arg(ITEM_PROMPT)
        .arg("--model").arg(model)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true);
    let mut child = cmd.spawn()?;
    if let Some(mut stdin) = child.stdin.take() { stdin.write_all(prompt.as_bytes()).await?; }
    let out = child.wait_with_output().await?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    let text = serde_json::from_str::<Value>(stdout.trim()).ok()
        .and_then(|e| e.get("result").and_then(|r| r.as_str()).map(str::to_string))
        .unwrap_or_else(|| stdout.to_string());
    let json = extract_json(&text).ok_or_else(|| anyhow::anyhow!("no JSON from item check"))?;
    #[derive(Deserialize)]
    struct Raw { ok: bool, #[serde(default)] note: String }
    let raw: Raw = serde_json::from_str(&json)?;
    Ok(ItemVerdict { line: item.line.clone(), ok: raw.ok, note: raw.note })
}

// ─────────────────────────── 4. Aggregate ───────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct FormReview {
    pub form_code: String,
    pub all_ok: bool,
    pub verdicts: Vec<ItemVerdict>,
}

impl FormReview {
    pub fn failures(&self) -> Vec<&ItemVerdict> { self.verdicts.iter().filter(|v| !v.ok).collect() }
}

/// Full review of one form: deterministic enumeration → per-item AI check → aggregate.
pub async fn review_form(model: &str, form_code: &str, pdf_path: &str, expected: &[Value]) -> FormReview {
    let items = enumerate_items(pdf_path, expected);
    let mut verdicts = Vec::with_capacity(items.len());
    for item in &items {
        let v = check_item(model, item).await.unwrap_or(ItemVerdict {
            line: item.line.clone(), ok: false, note: "item check failed to run".into(),
        });
        verdicts.push(v);
    }
    FormReview { form_code: form_code.to_string(), all_ok: verdicts.iter().all(|v| v.ok), verdicts }
}

fn extract_json(s: &str) -> Option<String> {
    let a = s.find('{')?;
    let b = s.rfind('}')?;
    (b > a).then(|| s[a..=b].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maven_needs_sch_l_hayat_exempt() {
        let maven = EntityFacts { kind: "c_corp".into(), state: "IL".into(), receipts: 294_700, total_assets: 324_233, rental: false, shareholders: 0, files_8832: false };
        let got = applicable_forms(&maven);
        assert!(got.iter().any(|f| f.code == "1120-SchL" && f.required));
        let hayat = EntityFacts { kind: "s_corp".into(), state: "IL".into(), receipts: 90_000, total_assets: 0, rental: false, shareholders: 1, files_8832: false };
        let got = applicable_forms(&hayat);
        assert!(got.iter().any(|f| f.code == "1120-S-SchL" && !f.required));
    }
}
