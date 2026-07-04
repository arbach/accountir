//! Owner signatures: stored per user (the owner) and stamped onto approved tax
//! forms before mailing. A signature is always kept as a stampable PNG, whether
//! it was uploaded as an image or typed and rendered in a handwriting font.

use crate::error::{AppError, AppResult};
use serde_json::{json, Value};
use sqlx::{PgPool, Row};
use uuid::Uuid;

/// Handwriting fonts offered for a typed signature: (key, display name).
/// The key maps to `<FONTS_DIR>/<key>.ttf` for the pdfform helper.
pub const FONTS: &[(&str, &str)] = &[
    ("GreatVibes", "Great Vibes"),
    ("DancingScript", "Dancing Script"),
    ("Allura", "Allura"),
    ("Sacramento", "Sacramento"),
    ("Pacifico", "Pacifico"),
    ("HomemadeApple", "Homemade Apple"),
];

pub fn is_valid_font(key: &str) -> bool {
    FONTS.iter().any(|(k, _)| *k == key)
}

fn pdfform_bin() -> String {
    std::env::var("PDFFORM_BIN").unwrap_or_else(|_| "/usr/local/lib/accountir/pdfform.py".to_string())
}

fn fonts_dir() -> String {
    std::env::var("FONTS_DIR").unwrap_or_else(|_| "/usr/local/lib/accountir/fonts".to_string())
}

/// Render a typed name in a handwriting font to a tight, transparent PNG.
pub fn render_typed(text: &str, font: &str) -> AppResult<Vec<u8>> {
    let text = text.trim();
    if text.is_empty() {
        return Err(AppError::BadRequest("signature text is empty".into()));
    }
    if text.len() > 120 {
        return Err(AppError::BadRequest("signature text is too long".into()));
    }
    if !is_valid_font(font) {
        return Err(AppError::BadRequest(format!("unknown font '{font}'")));
    }
    let spec = json!({ "text": text, "font": font, "height": 160 });
    let spec_path = std::env::temp_dir().join(format!("sig-{}.json", Uuid::new_v4()));
    let out_path = std::env::temp_dir().join(format!("sig-{}.png", Uuid::new_v4()));
    std::fs::write(&spec_path, serde_json::to_vec(&spec).unwrap_or_default())
        .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    let run = std::process::Command::new(pdfform_bin())
        .env("FONTS_DIR", fonts_dir())
        .args([
            "text2png",
            out_path.to_str().unwrap_or_default(),
            spec_path.to_str().unwrap_or_default(),
        ])
        .output();
    let _ = std::fs::remove_file(&spec_path);
    let out = run.map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    let v: Value = serde_json::from_str(stdout.trim()).unwrap_or_else(|_| json!({}));
    if let Some(err) = v.get("error").and_then(|e| e.as_str()) {
        let _ = std::fs::remove_file(&out_path);
        return Err(AppError::BadRequest(format!("signature render failed: {err}")));
    }
    let bytes = std::fs::read(&out_path).map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;
    let _ = std::fs::remove_file(&out_path);
    Ok(bytes)
}

/// Metadata about an owner's signature (no image bytes).
#[derive(Debug, Clone)]
pub struct SignatureMeta {
    pub kind: String,
    pub typed_text: Option<String>,
    pub typed_font: Option<String>,
    pub updated_at: String,
}

pub async fn get_meta(pool: &PgPool, user_id: Uuid) -> AppResult<Option<SignatureMeta>> {
    let row = sqlx::query(
        "SELECT kind, typed_text, typed_font, to_char(updated_at, 'YYYY-MM-DD HH24:MI')
         FROM owner_signatures WHERE user_id = $1",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| SignatureMeta {
        kind: r.get(0),
        typed_text: r.get(1),
        typed_font: r.get(2),
        updated_at: r.get(3),
    }))
}

/// The stampable PNG (or original uploaded image) and its content type.
pub async fn get_image(pool: &PgPool, user_id: Uuid) -> AppResult<Option<(Vec<u8>, String)>> {
    let row = sqlx::query("SELECT image_png, content_type FROM owner_signatures WHERE user_id = $1")
        .bind(user_id)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(|r| (r.get(0), r.get(1))))
}

pub async fn has_signature(pool: &PgPool, user_id: Uuid) -> bool {
    sqlx::query("SELECT 1 FROM owner_signatures WHERE user_id = $1")
        .bind(user_id)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
        .is_some()
}

async fn upsert(
    pool: &PgPool,
    user_id: Uuid,
    kind: &str,
    png: &[u8],
    content_type: &str,
    typed_text: Option<&str>,
    typed_font: Option<&str>,
) -> AppResult<()> {
    sqlx::query(
        "INSERT INTO owner_signatures (user_id, kind, image_png, content_type, typed_text, typed_font, updated_at)
         VALUES ($1, $2, $3, $4, $5, $6, now())
         ON CONFLICT (user_id) DO UPDATE
           SET kind = EXCLUDED.kind, image_png = EXCLUDED.image_png,
               content_type = EXCLUDED.content_type, typed_text = EXCLUDED.typed_text,
               typed_font = EXCLUDED.typed_font, updated_at = now()",
    )
    .bind(user_id)
    .bind(kind)
    .bind(png)
    .bind(content_type)
    .bind(typed_text)
    .bind(typed_font)
    .execute(pool)
    .await?;
    Ok(())
}

/// Save a typed signature: render it to PNG and store it.
pub async fn save_typed(pool: &PgPool, user_id: Uuid, text: &str, font: &str) -> AppResult<()> {
    let png = render_typed(text, font)?;
    upsert(pool, user_id, "typed", &png, "image/png", Some(text.trim()), Some(font)).await
}

/// Save an uploaded signature image (PNG/JPG).
pub async fn save_image(
    pool: &PgPool,
    user_id: Uuid,
    bytes: &[u8],
    content_type: &str,
) -> AppResult<()> {
    if bytes.is_empty() {
        return Err(AppError::BadRequest("empty image".into()));
    }
    if bytes.len() > 4 * 1024 * 1024 {
        return Err(AppError::BadRequest("image too large (max 4 MB)".into()));
    }
    let ct = match content_type {
        "image/png" | "image/jpeg" | "image/jpg" => content_type,
        _ => return Err(AppError::BadRequest("signature must be a PNG or JPEG image".into())),
    };
    upsert(pool, user_id, "image", bytes, ct, None, None).await
}

pub async fn clear(pool: &PgPool, user_id: Uuid) -> AppResult<()> {
    sqlx::query("DELETE FROM owner_signatures WHERE user_id = $1")
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}
