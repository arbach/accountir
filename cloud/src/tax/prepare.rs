//! Auto-preparation orchestrator — the agent-callable "do taxes for <entity> <year>".
//!
//! Pipeline (all deterministic except the per-item AI review):
//!   1. `applicable_forms` — which forms this entity must file.
//!   2. For each form: load its **formspec** (field → source), resolve each source
//!      from the tax profile + the computed return, and build the fill spec.
//!   3. `fill_form` (taxpdf.ts, by field name) writes the PDF + records the spec.
//!   4. `review_form_by_id` (deterministic loop + AI per-item) gates readiness.
//!
//! Formspecs (`FORMSPECS_DIR/<form_code>.json`) are the data that makes this
//! reproducible year-over-year — the field maps I derived become software.

use serde_json::{json, Map, Value};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::tax::review::{self, EntityFacts};

fn formspecs_dir() -> String {
    std::env::var("TAX_FORMSPECS_DIR")
        .unwrap_or_else(|_| "/usr/local/lib/accountir/tax/formspecs".to_string())
}

/// The computed return line-values for an entity-year (bridge/opentax output).
pub fn computed_lines(entity_key: &str, year: i32) -> AppResult<Value> {
    let out = std::env::var("BRIDGE_OUT")
        .unwrap_or_else(|_| "/var/lib/accountir-cloud/tax-out".to_string());
    let path = format!("{out}/{entity_key}_{year}_fill.json");
    let text = std::fs::read_to_string(&path)
        .map_err(|_| AppError::BadRequest(format!("no computed return at {path}; run compute first")))?;
    let v: Value = serde_json::from_str(&text)
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;
    Ok(v.get("lines").cloned().unwrap_or_else(|| json!({})))
}

/// Resolve one formspec source token to a value.
///   "profile.<path>"  -> value from the profile JSON (dotted path)
///   "compute.<line>"  -> value from the computed lines
///   "literal:<text>"  -> the literal text
fn resolve(src: &str, profile: &Value, computed: &Value) -> Option<Value> {
    if let Some(path) = src.strip_prefix("profile.") {
        // composite address helpers (the profile stores address as an object)
        if path == "address.line" || path == "address.csz" {
            let a = profile.get("address")?;
            let g = |k: &str| a.get(k).and_then(|v| v.as_str()).unwrap_or("");
            let s = if path == "address.line" {
                let (l1, l2) = (g("line1"), g("line2"));
                if l2.is_empty() { l1.to_string() } else { format!("{l1}, {l2}") }
            } else {
                format!("{}, {} {}", g("city"), g("state"), g("zip")).trim().to_string()
            };
            return Some(Value::String(s));
        }
        // FEIN split (IL forms take the first 2 + last 7 digits in separate boxes)
        if path == "ein.first2" || path == "ein.last7" {
            let digits: String = profile.get("ein").and_then(|v| v.as_str()).unwrap_or("")
                .chars().filter(|c| c.is_ascii_digit()).collect();
            let s = if path == "ein.first2" { digits.chars().take(2).collect() }
                    else { digits.chars().skip(2).collect() };
            return Some(Value::String(s));
        }
        let mut cur = profile;
        for seg in path.split('.') {
            cur = cur.get(seg)?;
        }
        Some(cur.clone())
    } else if let Some(line) = src.strip_prefix("compute.") {
        computed.get(line).cloned()
    } else if let Some(lit) = src.strip_prefix("literal:") {
        Some(Value::String(lit.to_string()))
    } else {
        None
    }
}

/// Build the taxpdf fill spec ({amounts, text, check}) for one form from its formspec.
pub fn build_fill_spec(form_code: &str, profile: &Value, computed: &Value) -> AppResult<Value> {
    let path = format!("{}/{form_code}.json", formspecs_dir());
    let spec: Value = std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .ok_or_else(|| AppError::BadRequest(format!("no formspec for '{form_code}' at {path}")))?;

    let mut text = Map::new();
    let mut amounts = Map::new();
    let mut check: Vec<Value> = Vec::new();

    if let Some(id) = spec.get("identity").and_then(|v| v.as_object()) {
        for (field, src) in id {
            if let Some(v) = src.as_str().and_then(|s| resolve(s, profile, computed)) {
                text.insert(field.clone(), v);
            }
        }
    }
    if let Some(am) = spec.get("amounts").and_then(|v| v.as_object()) {
        for (field, src) in am {
            if let Some(v) = src.as_str().and_then(|s| resolve(s, profile, computed)) {
                amounts.insert(field.clone(), v);
            }
        }
    }
    // checkboxes: "profile.accounting_method==cash" style conditions
    if let Some(cb) = spec.get("checkboxes").and_then(|v| v.as_object()) {
        for (field, cond) in cb {
            if let Some((src, want)) = cond.as_str().and_then(|c| c.split_once("==")) {
                if let Some(v) = resolve(src, profile, computed) {
                    if v.as_str() == Some(want) {
                        check.push(Value::String(field.clone()));
                    }
                }
            }
        }
    }
    Ok(json!({ "amounts": amounts, "text": text, "check": check }))
}

/// Map a tax profile + computed return into the facts the applicability engine needs.
fn facts_from(profile: &Value, computed: &Value) -> EntityFacts {
    let g = |k: &str| computed.get(k).and_then(|v| v.as_f64()).unwrap_or(0.0) as i64;
    EntityFacts {
        kind: profile.get("entity_type").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        state: profile.get("address").and_then(|a| a.get("state")).and_then(|v| v.as_str()).unwrap_or("").to_string(),
        receipts: g("gross_receipts"),
        total_assets: g("total_assets"),
        rental: computed.get("has_rental").and_then(|v| v.as_bool()).unwrap_or(false),
        shareholders: profile.get("shareholders").and_then(|v| v.as_u64()).unwrap_or(1) as u32,
        files_8832: profile.get("files_8832").and_then(|v| v.as_bool()).unwrap_or(false),
    }
}

/// One form's preparation outcome.
#[derive(serde::Serialize)]
pub struct PreparedForm {
    pub form_code: String,
    pub filled: bool,
    pub all_ok: bool,
    pub issues: Vec<String>,
}

/// End-to-end preparation of an entity's return. This is what the agent runs for
/// "do taxes for <entity> <year>": applicability → fill each required form from
/// its formspec → deterministic+AI review. Returns a per-form readiness summary.
pub async fn prepare_return(
    pool: &PgPool,
    company_id: Uuid,
    entity_key: &str,
    year: i32,
    profile: &Value,
    model: &str,
) -> AppResult<Vec<PreparedForm>> {
    let computed = computed_lines(entity_key, year)?;
    let forms = review::applicable_forms(&facts_from(profile, &computed));
    let mut out = Vec::new();
    for f in forms.iter().filter(|f| f.required) {
        // build the spec; skip forms without a formspec yet (reported, not silently dropped)
        let spec = match build_fill_spec(&f.code, profile, &computed) {
            Ok(s) => s,
            Err(e) => {
                out.push(PreparedForm { form_code: f.code.clone(), filled: false, all_ok: false, issues: vec![e.to_string()] });
                continue;
            }
        };
        // locate the form row (must have been fetched/registered)
        let row = sqlx::query_scalar::<_, Uuid>(
            "SELECT id FROM tax_forms WHERE company_id=$1 AND form_code=$2 AND year=$3 ORDER BY created_at DESC LIMIT 1",
        )
        .bind(company_id).bind(&f.code).bind(year)
        .fetch_optional(pool).await?;
        let Some(id) = row else {
            out.push(PreparedForm { form_code: f.code.clone(), filled: false, all_ok: false, issues: vec!["form not fetched yet".into()] });
            continue;
        };
        crate::tax::fill_form(pool, company_id, id, &spec).await?;
        let rev = crate::tax::review_form_by_id(pool, company_id, id, model).await?;
        out.push(PreparedForm {
            form_code: f.code.clone(),
            filled: true,
            all_ok: rev.all_ok,
            issues: rev.failures().iter().map(|v| format!("{}: {}", v.line, v.note)).collect(),
        });
    }
    Ok(out)
}
