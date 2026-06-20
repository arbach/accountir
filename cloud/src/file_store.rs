//! Per-company document store. Original bytes of every uploaded file are kept
//! on disk under `<FILES_DIR>/<company_id>/<sha256>` and indexed in the
//! `company_files` table, content-deduped by SHA-256 within a company.
//!
//! Shared by the Files page, the chat/agent upload path, and the cross-entity
//! `move_file` agent tool so the owner can drop a document into the personal
//! session and have it filed under whichever entity it actually belongs to.

use sqlx::PgPool;
use uuid::Uuid;

use crate::queries;

/// Root directory for stored files. Override with the `FILES_DIR` env var.
pub fn files_dir() -> String {
    std::env::var("FILES_DIR").unwrap_or_else(|_| "/var/lib/accountir-cloud/files".to_string())
}

fn company_dir(company_id: Uuid) -> std::path::PathBuf {
    std::path::Path::new(&files_dir()).join(company_id.to_string())
}

/// Best-effort detection of the period/tax year a document pertains to. Scans
/// the filename (and an optional text sample) for a plausible 4-digit year and
/// returns the most recent one in range. None when nothing convincing is found
/// — the user/agent can set it explicitly afterwards.
pub fn detect_year(filename: &str, text_sample: Option<&str>) -> Option<i32> {
    fn scan(s: &str, out: &mut Vec<i32>) {
        let bytes = s.as_bytes();
        let mut i = 0;
        while i + 4 <= bytes.len() {
            if bytes[i].is_ascii_digit()
                && bytes[i + 1].is_ascii_digit()
                && bytes[i + 2].is_ascii_digit()
                && bytes[i + 3].is_ascii_digit()
            {
                // Not part of a longer digit run (avoid matching inside a hash/id).
                let prev_digit = i > 0 && bytes[i - 1].is_ascii_digit();
                let next_digit = i + 4 < bytes.len() && bytes[i + 4].is_ascii_digit();
                if !prev_digit && !next_digit {
                    if let Ok(y) = s[i..i + 4].parse::<i32>() {
                        if (2000..=2099).contains(&y) {
                            out.push(y);
                        }
                    }
                }
                i += 4;
            } else {
                i += 1;
            }
        }
    }
    let mut years = Vec::new();
    scan(filename, &mut years);
    if years.is_empty() {
        if let Some(t) = text_sample {
            // Only the head of the text — statement period usually appears early.
            let head: String = t.chars().take(4000).collect();
            scan(&head, &mut years);
        }
    }
    years.into_iter().max()
}

/// Persist one uploaded file's original bytes into a company's file store,
/// content-deduped by SHA-256. Returns the file row's id (existing row's id on
/// duplicate), or None if the bytes were empty or storage failed.
#[allow(clippy::too_many_arguments)]
pub async fn store_company_file(
    pool: &PgPool,
    company_id: Uuid,
    category: &str,
    filename: &str,
    content_type: &str,
    bytes: &[u8],
    doc_year: Option<i32>,
) -> Option<Uuid> {
    use sha2::{Digest, Sha256};
    if bytes.is_empty() {
        return None;
    }
    let dir = company_dir(company_id);
    if tokio::fs::create_dir_all(&dir).await.is_err() {
        return None;
    }
    let sha = hex::encode(Sha256::digest(bytes));
    let path = dir.join(&sha);
    // Write the bytes even on dedup-by-content; the path is sha-addressed so a
    // re-write is idempotent. insert_company_file de-dupes the index row.
    if tokio::fs::write(&path, bytes).await.is_err() {
        return None;
    }
    queries::insert_company_file(
        pool, company_id, category, filename, content_type,
        bytes.len() as i64, &sha, &path.to_string_lossy(), doc_year,
    )
    .await
    .ok()
    .flatten()
}

/// Move a stored file from one entity to another: copies the bytes into the
/// target company's dir, indexes it there, then removes the source row + bytes.
/// Membership in both companies is the caller's responsibility to verify.
/// Returns the new file id under the target company, or an error string.
pub async fn move_company_file(
    pool: &PgPool,
    from_company: Uuid,
    to_company: Uuid,
    file_id: Uuid,
) -> Result<Uuid, String> {
    if from_company == to_company {
        return Err("source and target entity are the same".to_string());
    }
    let f = queries::get_company_file_full(pool, from_company, file_id)
        .await
        .map_err(|e| format!("lookup failed: {e}"))?
        .ok_or_else(|| "file not found under the source entity".to_string())?;

    let bytes = tokio::fs::read(&f.stored_path)
        .await
        .map_err(|e| format!("could not read stored bytes: {e}"))?;

    let dir = company_dir(to_company);
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|e| format!("could not create target dir: {e}"))?;
    let new_path = dir.join(&f.sha256);
    tokio::fs::write(&new_path, &bytes)
        .await
        .map_err(|e| format!("could not write to target: {e}"))?;

    let new_id = queries::insert_company_file(
        pool, to_company, &f.category, &f.filename, &f.content_type,
        f.size_bytes, &f.sha256, &new_path.to_string_lossy(), f.doc_year,
    )
    .await
    .map_err(|e| format!("could not index under target: {e}"))?
    .ok_or_else(|| "target index returned no id".to_string())?;

    // Remove the source index row, then its bytes (only if the path differs, so
    // we never delete the freshly written target copy when dirs happen to alias).
    if let Ok(Some(old_path)) = queries::delete_company_file(pool, from_company, file_id).await {
        if old_path != new_path.to_string_lossy() {
            let _ = tokio::fs::remove_file(&old_path).await;
        }
    }
    Ok(new_id)
}
