//! Tax filing pipeline: pull official IRS form PDFs, fill them (pypdf helper),
//! gate on user approval, and mail them via Lob. Forms live on disk under
//! TAX_FORMS_DIR; metadata + status in the tax_forms table.
//!
//! Step process: profile -> books review -> pull -> fill -> approve (UI) -> mail.
//! The mail step refuses anything not user-approved.

pub mod lob;

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

fn pdfform_bin() -> String {
    std::env::var("PDFFORM_BIN").unwrap_or_else(|_| "/usr/local/lib/accountir/pdfform.py".to_string())
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
}

impl TaxFormRow {
    pub fn mailed_display(&self) -> String {
        self.mailed_at
            .map(|t| t.format("%Y-%m-%d %H:%M UTC").to_string())
            .unwrap_or_default()
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
    }
}

const FORM_COLS: &str =
    "id, year, form_code, title, status, file_path, lob_id, lob_status, mailed_at";

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

pub async fn approve_form(pool: &PgPool, company_id: Uuid, id: Uuid) -> AppResult<()> {
    update_status(pool, company_id, id, "approved").await
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

fn run_pdfform(args: &[&str]) -> Result<Value, String> {
    let out = std::process::Command::new(pdfform_bin())
        .args(args)
        .output()
        .map_err(|e| format!("pdfform helper failed to start: {e}"))?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    let v: Value = serde_json::from_str(stdout.trim())
        .map_err(|_| format!("pdfform helper bad output: {}", stdout.chars().take(200).collect::<String>()))?;
    if let Some(err) = v.get("error").and_then(|e| e.as_str()) {
        return Err(err.to_string());
    }
    Ok(v)
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

    let fields = run_pdfform(&["list", path.to_str().unwrap_or_default()])
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
    run_pdfform(&["list", &form.file_path]).map_err(AppError::BadRequest)
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
    let res = run_pdfform(&[
        "fill",
        &form.file_path,
        &out_path,
        tmp.to_str().unwrap_or_default(),
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
    if form.status != "approved" {
        return Err(AppError::BadRequest(
            "form is not approved yet — the user must review the PDF and click Approve on the Tax Filing page before it can be mailed"
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
