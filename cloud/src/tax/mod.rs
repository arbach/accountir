//! Tax filing pipeline: pull official IRS form PDFs, fill them by AcroForm field
//! name via the taxpdf.ts helper (deterministic placement),
//! gate on user approval, and mail them via Lob. Forms live on disk under
//! TAX_FORMS_DIR; metadata + status in the tax_forms table.
//!
//! Step process: profile -> books review -> pull -> fill -> approve (UI) -> mail.
//! The mail step refuses anything not user-approved.

pub mod lob;
pub mod runtime;
pub mod review;
pub mod prepare;

use std::path::PathBuf;

use serde_json::{json, Value};
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::store::event_store::set_tenant;

pub fn forms_dir() -> PathBuf {
    std::env::var("TAX_FORMS_DIR")
        .unwrap_or_else(|_| "/var/lib/accountir-cloud/tax-forms".to_string())
        .into()
}

fn taxpdf_bin() -> String {
    std::env::var("TAXPDF_BIN").unwrap_or_else(|_| "/usr/local/lib/accountir/tax/taxpdf.ts".to_string())
}

#[derive(Debug, Clone)]
pub struct TaxProfile {
    pub entity_type: String,
    pub legal_name: String,
    pub ein: String,
    pub address: Value,
}

pub async fn get_profile(pool: &PgPool, company_id: Uuid) -> AppResult<Option<TaxProfile>> {
    let mut conn = pool.acquire().await?;
    let mut tx = sqlx::Acquire::begin(&mut conn).await?;
    set_tenant(&mut tx, company_id).await?;
    let row: Option<(String, String, String, Value)> = sqlx::query_as(
        "SELECT entity_type, legal_name, ein, address FROM tax_profiles WHERE company_id = $1",
    )
    .bind(company_id)
    .fetch_optional(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(row.map(|(entity_type, legal_name, ein, address)| TaxProfile {
        entity_type,
        legal_name,
        ein,
        address,
    }))
}

pub async fn set_profile(
    pool: &PgPool,
    company_id: Uuid,
    entity_type: &str,
    legal_name: &str,
    ein: &str,
    address: &Value,
) -> AppResult<()> {
    let mut conn = pool.acquire().await?;
    let mut tx = sqlx::Acquire::begin(&mut conn).await?;
    set_tenant(&mut tx, company_id).await?;
    sqlx::query(
        "INSERT INTO tax_profiles (company_id, entity_type, legal_name, ein, address)
         VALUES ($1, $2, $3, $4, $5)
         ON CONFLICT (company_id) DO UPDATE
           SET entity_type = $2, legal_name = $3, ein = $4, address = $5, updated_at = now()",
    )
    .bind(company_id)
    .bind(entity_type)
    .bind(legal_name)
    .bind(ein)
    .bind(address)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}

const ENTITY_DOC_SYSTEM: &str = "You extract business identity data from US entity/tax documents \
(IRS EIN assignment letter CP-575, articles of organization/incorporation, S-election CP261, \
prior-year tax returns, state registrations). Respond with ONLY a JSON object, no prose: \
{\"entity_type\": \"schedule_c\"|\"s_corp\"|\"partnership\"|\"c_corp\"|null, \"legal_name\": string|null, \
\"ein\": \"XX-XXXXXXX\"|null, \"address\": {\"line1\": string, \"line2\": string, \"city\": string, \
\"state\": string, \"zip\": string}|null}. \
entity_type mapping: Form 1120-S or accepted S election -> s_corp; Form 1065 or partnership -> partnership; \
Form 1120 -> c_corp; Schedule C, sole proprietor, or single-member LLC -> schedule_c. \
Use null for anything the document does not state — never guess an EIN.";

/// AI-parse an uploaded entity document into tax-profile fields via the agent
/// daemon's stateless /oneshot endpoint.
pub async fn parse_entity_document(text: &str) -> Result<Value, String> {
    let bounded: String = text.chars().take(40_000).collect();
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .map_err(|e| format!("http client: {e}"))?;
    let resp = client
        .post(format!("{}/oneshot", crate::ai::agent::agentd_url()))
        .json(&json!({
            "system": ENTITY_DOC_SYSTEM,
            "prompt": format!("Document text:\n\n{bounded}"),
        }))
        .send()
        .await
        .map_err(|e| format!("agent daemon unreachable: {e}"))?;
    let body: Value = resp.json().await.map_err(|e| format!("bad daemon reply: {e}"))?;
    if !body["ok"].as_bool().unwrap_or(false) {
        return Err(format!(
            "agent parse failed: {}",
            body["error"].as_str().unwrap_or("unknown")
        ));
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
    serde_json::from_str::<Value>(json_str).map_err(|e| {
        format!(
            "could not parse AI response: {e}; got: {}",
            out.chars().take(200).collect::<String>()
        )
    })
}

#[derive(Debug, Clone)]
pub struct TaxFormRow {
    pub id: Uuid,
    pub year: i32,
    pub form_code: String,
    pub title: String,
    pub status: String,
    pub file_path: String,
    pub lob_id: Option<String>,
    pub lob_status: Option<String>,
    pub mailed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub signed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub signed_by: Option<String>,
}

impl TaxFormRow {
    pub fn mailed_display(&self) -> String {
        self.mailed_at
            .map(|t| t.format("%Y-%m-%d %H:%M UTC").to_string())
            .unwrap_or_default()
    }
    pub fn signed_display(&self) -> String {
        match (&self.signed_at, &self.signed_by) {
            (Some(t), Some(by)) => format!("{} · {}", by, t.format("%Y-%m-%d")),
            (Some(t), None) => t.format("%Y-%m-%d").to_string(),
            _ => String::new(),
        }
    }
}

fn form_row(r: sqlx::postgres::PgRow) -> TaxFormRow {
    TaxFormRow {
        id: r.get(0),
        year: r.get(1),
        form_code: r.get(2),
        title: r.get(3),
        status: r.get(4),
        file_path: r.get(5),
        lob_id: r.get(6),
        lob_status: r.get(7),
        mailed_at: r.get(8),
        signed_at: r.get(9),
        signed_by: r.get(10),
    }
}

const FORM_COLS: &str =
    "id, year, form_code, title, status, file_path, lob_id, lob_status, mailed_at, signed_at, signed_by";

pub async fn list_forms(pool: &PgPool, company_id: Uuid) -> AppResult<Vec<TaxFormRow>> {
    let mut conn = pool.acquire().await?;
    let mut tx = sqlx::Acquire::begin(&mut conn).await?;
    set_tenant(&mut tx, company_id).await?;
    let rows = sqlx::query(&format!(
        "SELECT {FORM_COLS} FROM tax_forms WHERE company_id = $1 ORDER BY year DESC, created_at DESC"
    ))
    .bind(company_id)
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(rows.into_iter().map(form_row).collect())
}

pub async fn get_form(pool: &PgPool, company_id: Uuid, id: Uuid) -> AppResult<Option<TaxFormRow>> {
    let mut conn = pool.acquire().await?;
    let mut tx = sqlx::Acquire::begin(&mut conn).await?;
    set_tenant(&mut tx, company_id).await?;
    let row = sqlx::query(&format!("SELECT {FORM_COLS} FROM tax_forms WHERE id = $1"))
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(row.map(form_row))
}

async fn update_status(
    pool: &PgPool,
    company_id: Uuid,
    id: Uuid,
    status: &str,
) -> AppResult<()> {
    let mut conn = pool.acquire().await?;
    let mut tx = sqlx::Acquire::begin(&mut conn).await?;
    set_tenant(&mut tx, company_id).await?;
    sqlx::query("UPDATE tax_forms SET status = $2, updated_at = now() WHERE id = $1")
        .bind(id)
        .bind(status)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

/// Step-4 review gate: deterministically loop every intended field on the form
/// (from the booking-derived fill spec stored in `fields`), read what is ACTUALLY
/// on the PDF, and have the AI judge each item's correctness. A form can only be
/// approved when every item passes. Optional `{form_code}.json` under the specs
/// dir maps opaque field names to human line labels for a sharper AI judgment.
pub async fn review_form_by_id(
    pool: &PgPool,
    company_id: Uuid,
    id: Uuid,
    model: &str,
) -> AppResult<review::FormReview> {
    let form = get_form(pool, company_id, id).await?.ok_or(AppError::NotFound)?;
    // intended values = the booking-derived fill spec applied to this form
    let spec: Value = sqlx::query_scalar::<_, Value>(
        "SELECT COALESCE(fields, '{}'::jsonb) FROM tax_forms WHERE id = $1",
    )
    .bind(id)
    .fetch_one(pool)
    .await
    .unwrap_or_else(|_| json!({}));
    // optional field -> line-label map for sharper judgments
    let specs_dir = std::env::var("TAX_FORMSPECS_DIR")
        .unwrap_or_else(|_| "/usr/local/lib/accountir/tax/formspecs".to_string());
    let labels: Value = std::fs::read_to_string(
        std::path::Path::new(&specs_dir).join(format!("{}.json", form.form_code)),
    )
    .ok()
    .and_then(|s| serde_json::from_str(&s).ok())
    .unwrap_or_else(|| json!({}));
    let label_of = |field: &str| -> String {
        labels.get(field).and_then(|v| v.as_str()).unwrap_or(field).to_string()
    };
    // build the deterministic item list from the intended spec
    let mut items: Vec<Value> = Vec::new();
    for (field, v) in spec.get("amounts").and_then(|a| a.as_object()).into_iter().flatten() {
        items.push(json!({ "line": label_of(field), "field": field, "expected": v, "kind": "amount" }));
    }
    for (field, v) in spec.get("text").and_then(|a| a.as_object()).into_iter().flatten() {
        items.push(json!({ "line": label_of(field), "field": field, "expected": v, "kind": "text" }));
    }
    if let Some(arr) = spec.get("check").and_then(|c| c.as_array()) {
        for field in arr.iter().filter_map(|c| c.as_str()) {
            items.push(json!({ "line": label_of(field), "field": field, "checkbox": field, "kind": "checkbox" }));
        }
    }
    Ok(review::review_form(model, &form.form_code, &form.file_path, &items).await)
}

pub async fn approve_form(pool: &PgPool, company_id: Uuid, id: Uuid) -> AppResult<()> {
    // HARD GATE: the deterministic-loop + per-item AI review must pass first.
    let model = std::env::var("TAX_REVIEW_MODEL").unwrap_or_else(|_| "sonnet".to_string());
    let review = review_form_by_id(pool, company_id, id, &model).await?;
    if !review.all_ok {
        let fails: Vec<String> = review.failures().iter().map(|f| format!("{}: {}", f.line, f.note)).collect();
        return Err(AppError::BadRequest(format!(
            "review failed ({} issue(s)): {}",
            fails.len(),
            fails.join("; ")
        )));
    }
    update_status(pool, company_id, id, "approved").await
}

/// Where the signature image + date land on a form, in PDF points (origin
/// bottom-left). `page` may be negative to count from the end (-1 = last page).
/// These are best-effort defaults and are meant to be calibrated per form.
struct SignAnchor {
    page: i64,
    x: f64,
    y: f64,
    w: f64,
    date_x: f64,
    date_y: f64,
}

fn signature_anchor(form_code: &str) -> SignAnchor {
    // Coordinates in PDF points; calibrated against the official 2025 IRS PDFs.
    // Personal returns sign in the "Sign Here" block on page 2.
    if form_code.starts_with("f1040") || form_code == "il1040" {
        return SignAnchor { page: 1, x: 74.0, y: 126.0, w: 145.0, date_x: 312.0, date_y: 130.0 };
    }
    // Business returns (1120-S, 1120, 1065, IL variants) sign on the "Signature
    // of officer" line near the bottom of page 1.
    if form_code.starts_with("f1120")
        || form_code.starts_with("f1065")
        || form_code.starts_with("il1120")
    {
        return SignAnchor { page: 0, x: 112.0, y: 86.0, w: 150.0, date_x: 360.0, date_y: 92.0 };
    }
    // Fallback: bottom-left of the last page (calibrate per form as needed).
    SignAnchor { page: -1, x: 112.0, y: 86.0, w: 150.0, date_x: 360.0, date_y: 92.0 }
}

/// Stamp the owner's signature image + today's date onto an approved form and
/// mark it signed. HARD GATE: the form must be user-approved first.
pub async fn sign_form(
    pool: &PgPool,
    company_id: Uuid,
    id: Uuid,
    signature_png: &[u8],
    signer: &str,
) -> AppResult<()> {
    let form = get_form(pool, company_id, id).await?.ok_or(AppError::NotFound)?;
    if form.status == "mailed" {
        return Err(AppError::BadRequest("form already mailed".into()));
    }
    if form.status != "approved" {
        return Err(AppError::BadRequest(
            "form must be approved before it can be signed".into(),
        ));
    }

    // How many pages? Needed to resolve a from-the-end page index.
    let listed = run_taxpdf(&["list", &form.file_path]).map_err(AppError::BadRequest)?;
    let pages = listed.get("pages").and_then(|p| p.as_i64()).unwrap_or(1).max(1);
    let a = signature_anchor(&form.form_code);
    let page = if a.page < 0 { (pages + a.page).max(0) } else { a.page.min(pages - 1) };

    let sig_path = std::env::temp_dir().join(format!("taxsig-{id}.png"));
    std::fs::write(&sig_path, signature_png).map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;
    let today = chrono::Utc::now().date_naive().format("%m/%d/%Y").to_string();
    let spec = json!({
        "stamps": [{ "page": page, "x": a.x, "y": a.y, "w": a.w,
                     "image": sig_path.to_str().unwrap_or_default() }],
        "texts":  [{ "page": page, "x": a.date_x, "y": a.date_y, "text": today, "size": 10 }],
    });
    let spec_path = std::env::temp_dir().join(format!("taxsig-{id}.json"));
    std::fs::write(&spec_path, serde_json::to_vec(&spec).unwrap_or_default())
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;
    let out_path = format!("{}.signed.pdf", form.file_path);
    let res = run_taxpdf(&["stamp", &form.file_path, spec_path.to_str().unwrap_or_default(), &out_path]);
    let _ = std::fs::remove_file(&sig_path);
    let _ = std::fs::remove_file(&spec_path);
    res.map_err(AppError::BadRequest)?;
    std::fs::rename(&out_path, &form.file_path).map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    let mut conn = pool.acquire().await?;
    let mut tx = sqlx::Acquire::begin(&mut conn).await?;
    set_tenant(&mut tx, company_id).await?;
    sqlx::query(
        "UPDATE tax_forms SET status = 'signed', signed_at = now(), signed_by = $2, updated_at = now()
         WHERE id = $1",
    )
    .bind(id)
    .bind(signer)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}

pub async fn delete_form(pool: &PgPool, company_id: Uuid, id: Uuid) -> AppResult<()> {
    if let Some(f) = get_form(pool, company_id, id).await? {
        let _ = std::fs::remove_file(&f.file_path);
    }
    let mut conn = pool.acquire().await?;
    let mut tx = sqlx::Acquire::begin(&mut conn).await?;
    set_tenant(&mut tx, company_id).await?;
    sqlx::query("DELETE FROM tax_forms WHERE id = $1").bind(id).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(())
}

/// Run the single Deno PDF helper (`taxpdf.ts`): fill | dump | list | stamp.
/// Field-name filling (deterministic placement) — replaces the old pdfform.py.
fn run_taxpdf(args: &[&str]) -> Result<Value, String> {
    let deno = std::env::var("DENO_BIN").unwrap_or_else(|_| "/usr/local/bin/deno".to_string());
    let bin = taxpdf_bin();
    let mut full: Vec<&str> = vec!["run", "--allow-read", "--allow-write", &bin];
    full.extend_from_slice(args);
    let out = runtime::run("taxpdf", &deno, &full)?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    // pdf-lib prints a benign XFA notice on stdout; take the last JSON line.
    let line = stdout
        .lines()
        .rev()
        .find(|l| l.trim_start().starts_with('{'))
        .unwrap_or_else(|| stdout.trim());
    let v: Value = serde_json::from_str(line.trim())
        .map_err(|_| format!("taxpdf helper bad output: {}", stdout.chars().take(200).collect::<String>()))?;
    if let Some(err) = v.get("error").and_then(|e| e.as_str()) {
        return Err(err.to_string());
    }
    Ok(v)
}

// ─── OpenTax engine of record (deterministic compute) ──────────────────────────

/// Result of a deterministic OpenTax compute for one entity-year.
pub struct ComputeResult {
    pub form: String,
    pub reconciles: bool,
    pub computed: f64,
    pub lines: usize,
}

fn bridge_dir() -> String {
    std::env::var("BRIDGE_DIR").unwrap_or_else(|_| "/usr/local/lib/accountir/tax/bridge".to_string())
}

/// Map a company (its slug and/or legal name) to the OpenTax bridge entity key.
pub fn bridge_entity_key(text: &str) -> Option<&'static str> {
    let s = text.to_lowercase();
    if s.contains("sweethome") || s.contains("sweet home") {
        Some("sweethome")
    } else if s.contains("hayat") {
        Some("hayat")
    } else if s.contains("maven") {
        Some("maven")
    } else if s.contains("on-chain") || s.contains("onchain") {
        Some("on-chain")
    } else if s.contains("arbach") || s.contains("michael") {
        Some("michael")
    } else {
        None
    }
}

/// Run the OpenTax bridge (step4.py --fill) for one entity-year and parse its
/// output — the deterministic calculation of record: ledger + tax-line tags →
/// engine → reconciled line values. Blocking (shells to python/deno); call from
/// spawn_blocking. DATABASE_URL is inherited from the app process environment.
pub fn compute_return(entity_key: &str, year: i32) -> Result<ComputeResult, String> {
    let out_dir =
        std::env::var("BRIDGE_OUT").unwrap_or_else(|_| "/var/lib/accountir-cloud/tax-out".to_string());
    let deno_dir =
        std::env::var("DENO_DIR").unwrap_or_else(|_| "/var/lib/accountir-cloud/.deno".to_string());
    // Corporates compute from the ledger + tax-line tags (step4.py). An individual
    // 1040 aggregates source-document K-1s/1099s/8949, so it computes from the
    // source-grounded map (export_return.py). Both emit the same fill JSON.
    let (script, extra): (&str, &[&str]) = if entity_key == "michael" {
        ("export_return.py", &["--compute"])
    } else {
        ("step4.py", &[])
    };
    let _ = (&out_dir, &deno_dir); // env now applied centrally by runtime::run
    let script_path = format!("{}/{script}", bridge_dir());
    let year_s = year.to_string();
    let mut args: Vec<&str> = vec![
        &script_path,
        "--entity", entity_key,
        "--year", &year_s,
        "--fill",
    ];
    args.extend_from_slice(extra);
    let output = runtime::run("compute.bridge", "python3", &args)?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let msg: String = stderr
            .lines()
            .rev()
            .find(|l| !l.trim().is_empty())
            .unwrap_or("engine error")
            .chars()
            .take(200)
            .collect();
        return Err(msg);
    }
    let path = format!("{out_dir}/{entity_key}_{year}_fill.json");
    let data = std::fs::read_to_string(&path).map_err(|e| format!("no engine output: {e}"))?;
    let v: Value = serde_json::from_str(&data).map_err(|e| format!("bad engine output: {e}"))?;
    Ok(ComputeResult {
        form: v.get("form").and_then(|x| x.as_str()).unwrap_or("").to_string(),
        reconciles: v.get("reconciles").and_then(|x| x.as_bool()).unwrap_or(false),
        computed: v.get("computed").and_then(|x| x.as_f64()).unwrap_or(0.0),
        lines: v.get("lines").and_then(|x| x.as_object()).map(|o| o.len()).unwrap_or(0),
    })
}

/// Sanity-check a form code before it becomes part of a URL/path:
/// IRS form file names are short lowercase alphanumerics (f1040sc, f1120s…).
fn valid_form_code(code: &str) -> bool {
    !code.is_empty()
        && code.len() <= 24
        && code.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
}

/// Download an official IRS form PDF and register it. Returns (row id, field list).
pub async fn fetch_form(
    pool: &PgPool,
    company_id: Uuid,
    form_code: &str,
    year: i32,
    title: Option<&str>,
) -> AppResult<(Uuid, Value)> {
    if !valid_form_code(form_code) {
        return Err(AppError::BadRequest(format!(
            "invalid form code '{form_code}' (expected like f1040sc, f1120s, f1099nec)"
        )));
    }
    let url = format!("https://www.irs.gov/pub/irs-pdf/{form_code}.pdf");
    let resp = reqwest::get(&url)
        .await
        .map_err(|e| AppError::BadRequest(format!("irs.gov fetch failed: {e}")))?;
    if !resp.status().is_success() {
        return Err(AppError::BadRequest(format!(
            "irs.gov returned {} for {url} — check the form code",
            resp.status()
        )));
    }
    let bytes = resp
        .bytes()
        .await
        .map_err(|e| AppError::BadRequest(format!("irs.gov read failed: {e}")))?;
    if !bytes.starts_with(b"%PDF") {
        return Err(AppError::BadRequest("irs.gov did not return a PDF".to_string()));
    }

    let id = Uuid::new_v4();
    let dir = forms_dir().join(company_id.to_string());
    std::fs::create_dir_all(&dir).map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;
    let path = dir.join(format!("{form_code}-{year}-{id}.pdf"));
    std::fs::write(&path, &bytes).map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    let fields = run_taxpdf(&["list", path.to_str().unwrap_or_default()])
        .map_err(AppError::BadRequest)?;

    let title = title
        .map(str::to_string)
        .unwrap_or_else(|| format!("{} ({year})", form_code.to_uppercase()));
    let mut conn = pool.acquire().await?;
    let mut tx = sqlx::Acquire::begin(&mut conn).await?;
    set_tenant(&mut tx, company_id).await?;
    sqlx::query(
        "INSERT INTO tax_forms (id, company_id, year, form_code, title, file_path)
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(id)
    .bind(company_id)
    .bind(year)
    .bind(form_code)
    .bind(&title)
    .bind(path.to_str().unwrap_or_default())
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok((id, fields))
}

/// List the AcroForm fields of a pulled form.
pub async fn form_fields(pool: &PgPool, company_id: Uuid, id: Uuid) -> AppResult<Value> {
    let form = get_form(pool, company_id, id).await?.ok_or(AppError::NotFound)?;
    run_taxpdf(&["list", &form.file_path]).map_err(AppError::BadRequest)
}

/// Fill fields into the form PDF (in place, preserving the original download as
/// the fill source each time would lose prior values — we fill cumulatively).
pub async fn fill_form(
    pool: &PgPool,
    company_id: Uuid,
    id: Uuid,
    values: &Value,
) -> AppResult<Value> {
    let form = get_form(pool, company_id, id).await?.ok_or(AppError::NotFound)?;
    if form.status == "mailed" {
        return Err(AppError::BadRequest("form already mailed; pull a fresh copy".into()));
    }
    let tmp = std::env::temp_dir().join(format!("taxfill-{id}.json"));
    std::fs::write(&tmp, serde_json::to_vec(values).unwrap_or_default())
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;
    let out_path = format!("{}.tmp.pdf", form.file_path);
    let res = run_taxpdf(&[
        "fill",
        &form.file_path,
        tmp.to_str().unwrap_or_default(),
        &out_path,
    ])
    .map_err(AppError::BadRequest)?;
    let _ = std::fs::remove_file(&tmp);
    std::fs::rename(&out_path, &form.file_path).map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    let mut conn = pool.acquire().await?;
    let mut tx = sqlx::Acquire::begin(&mut conn).await?;
    set_tenant(&mut tx, company_id).await?;
    sqlx::query(
        "UPDATE tax_forms SET status = 'filled',
            fields = COALESCE(fields, '{}'::jsonb) || $2::jsonb, updated_at = now()
         WHERE id = $1",
    )
    .bind(id)
    .bind(values)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(res)
}

/// Mail a form via Lob. HARD GATE: the form must be user-approved in the UI.
pub async fn mail_form(
    pool: &PgPool,
    company_id: Uuid,
    id: Uuid,
    to: &Value,
    certified: bool,
) -> AppResult<Value> {
    let form = get_form(pool, company_id, id).await?.ok_or(AppError::NotFound)?;
    if form.status == "mailed" {
        return Err(AppError::BadRequest(format!(
            "already mailed (lob id {})",
            form.lob_id.unwrap_or_default()
        )));
    }
    if form.status != "signed" {
        return Err(AppError::BadRequest(
            "form is not signed yet — the user must review & Approve the PDF and then Sign it (step 6) on the Tax Filing page before it can be mailed"
                .into(),
        ));
    }
    let profile = get_profile(pool, company_id).await?.ok_or_else(|| {
        AppError::BadRequest("tax profile (legal name, address) is not set".into())
    })?;
    let from = json!({
        "name": profile.legal_name,
        "address_line1": profile.address.get("line1").and_then(|v| v.as_str()).unwrap_or(""),
        "address_line2": profile.address.get("line2").and_then(|v| v.as_str()).unwrap_or(""),
        "address_city": profile.address.get("city").and_then(|v| v.as_str()).unwrap_or(""),
        "address_state": profile.address.get("state").and_then(|v| v.as_str()).unwrap_or(""),
        "address_zip": profile.address.get("zip").and_then(|v| v.as_str()).unwrap_or(""),
    });
    let pdf = std::fs::read(&form.file_path).map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;
    let desc = format!("{} {} — {}", form.form_code, form.year, profile.legal_name);
    let letter = lob::send_letter(&pdf, to, &from, &desc, certified)
        .await
        .map_err(AppError::BadRequest)?;

    let lob_id = letter.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let lob_status = letter
        .get("expected_delivery_date")
        .and_then(|v| v.as_str())
        .map(|d| format!("expected delivery {d}"))
        .unwrap_or_else(|| "submitted".to_string());
    let mut conn = pool.acquire().await?;
    let mut tx = sqlx::Acquire::begin(&mut conn).await?;
    set_tenant(&mut tx, company_id).await?;
    sqlx::query(
        "UPDATE tax_forms SET status = 'mailed', lob_id = $2, lob_status = $3,
            to_address = $4, mailed_at = now(), updated_at = now() WHERE id = $1",
    )
    .bind(id)
    .bind(&lob_id)
    .bind(&lob_status)
    .bind(to)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(letter)
}
