//! Plaid Statements parsing: take a downloaded statement PDF, extract its text,
//! and use Claude to turn it into structured transaction lines.
//!
//! Bank statement PDFs have no standard layout, so heuristic parsing is brittle.
//! We lean on the LLM (the same Anthropic client the chat feature uses) to extract
//! a normalized list of transactions, which are then staged for user review.

use serde::Deserialize;
use serde_json::{json, Value};

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

/// Extract text from a PDF: poppler's pdftotext first (fast and robust —
/// pdf-extract panics on many real bank statements), then pdf-extract as a
/// fallback, then OCR (pdftoppm + tesseract) for scanned/image-only documents.
pub async fn extract_text_or_ocr(pdf: &[u8]) -> Result<String, String> {
    // A real text layer yields far more than stray whitespace/page numbers.
    if let Ok(t) = pdftotext(pdf).await {
        if t.trim().len() >= 50 {
            return Ok(t);
        }
    }
    if let Ok(t) = extract_text(pdf) {
        if t.trim().len() >= 50 {
            return Ok(t);
        }
    }
    ocr_pdf(pdf).await
}

/// poppler `pdftotext -layout` via stdin/stdout.
async fn pdftotext(pdf: &[u8]) -> Result<String, String> {
    use tokio::io::AsyncWriteExt;
    let mut child = tokio::process::Command::new("pdftotext")
        .args(["-layout", "-", "-"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| format!("pdftotext failed to start: {e}"))?;
    if let Some(mut stdin) = child.stdin.take() {
        let bytes = pdf.to_vec();
        // Write in a task so a full stdout pipe can't deadlock us.
        tokio::spawn(async move {
            let _ = stdin.write_all(&bytes).await;
        });
    }
    let out = child
        .wait_with_output()
        .await
        .map_err(|e| format!("pdftotext failed: {e}"))?;
    if !out.status.success() {
        return Err(format!("pdftotext exited with {}", out.status));
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

const OCR_MAX_PAGES: u32 = 25;

/// OCR a raster image (PNG/JPEG/GIF) directly with tesseract.
pub async fn ocr_image(bytes: &[u8]) -> Result<String, String> {
    let dir = std::env::temp_dir().join(format!("ocr-img-{}", uuid::Uuid::new_v4()));
    tokio::fs::create_dir_all(&dir).await.map_err(|e| format!("ocr tmp dir: {e}"))?;
    let input = dir.join("input");
    let res = async {
        tokio::fs::write(&input, bytes).await.map_err(|e| format!("ocr write: {e}"))?;
        let out = tokio::process::Command::new("tesseract")
            .arg(&input)
            .arg("stdout")
            .arg("--psm")
            .arg("6")
            .output()
            .await
            .map_err(|e| format!("tesseract failed to start: {e}"))?;
        if !out.status.success() {
            return Err(format!(
                "tesseract failed: {}",
                String::from_utf8_lossy(&out.stderr).chars().take(200).collect::<String>()
            ));
        }
        let text = String::from_utf8_lossy(&out.stdout).to_string();
        if text.trim().is_empty() {
            return Err("no readable text found in the image".into());
        }
        Ok(text)
    }
    .await;
    let _ = tokio::fs::remove_dir_all(&dir).await;
    res
}

async fn ocr_pdf(pdf: &[u8]) -> Result<String, String> {
    let dir = std::env::temp_dir().join(format!("ocr-{}", uuid::Uuid::new_v4()));
    tokio::fs::create_dir_all(&dir).await.map_err(|e| format!("ocr tmp dir: {e}"))?;
    let result = ocr_pdf_in(&dir, pdf).await;
    let _ = tokio::fs::remove_dir_all(&dir).await;
    result
}

async fn ocr_pdf_in(dir: &std::path::Path, pdf: &[u8]) -> Result<String, String> {
    let input = dir.join("input.pdf");
    tokio::fs::write(&input, pdf).await.map_err(|e| format!("ocr write: {e}"))?;

    let ppm = tokio::process::Command::new("pdftoppm")
        .args(["-r", "200", "-gray", "-png", "-l", &OCR_MAX_PAGES.to_string()])
        .arg(&input)
        .arg(dir.join("page"))
        .output()
        .await
        .map_err(|e| format!("pdftoppm failed to start: {e}"))?;
    if !ppm.status.success() {
        return Err(format!(
            "pdftoppm failed: {}",
            String::from_utf8_lossy(&ppm.stderr).chars().take(200).collect::<String>()
        ));
    }

    let mut pages: Vec<std::path::PathBuf> = std::fs::read_dir(dir)
        .map_err(|e| format!("ocr readdir: {e}"))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().map(|x| x == "png").unwrap_or(false))
        .collect();
    pages.sort();
    if pages.is_empty() {
        return Err("the PDF appears to be scanned, but no pages could be rendered for OCR".into());
    }

    let mut text = String::new();
    for page in &pages {
        let out = tokio::process::Command::new("tesseract")
            .arg(page)
            .arg("stdout")
            .arg("--psm")
            .arg("6")
            .output()
            .await
            .map_err(|e| format!("tesseract failed to start: {e}"))?;
        if out.status.success() {
            text.push_str(&String::from_utf8_lossy(&out.stdout));
            text.push('\n');
        }
    }
    if text.trim().is_empty() {
        return Err("OCR found no readable text in the scanned document".into());
    }
    tracing::info!(pages = pages.len(), chars = text.len(), "OCR-extracted scanned PDF");
    Ok(text)
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

/// Parse statement text into transaction lines using the Claude CLI via the
/// agent daemon's stateless /oneshot endpoint (subscription auth, no API key).
pub async fn parse_with_ai(statement_text: &str) -> Result<Vec<ParsedLine>, String> {
    // Bound input to keep the request within sane size/cost limits.
    let text: String = statement_text.chars().take(60_000).collect();
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(620))
        .build()
        .map_err(|e| format!("http client: {e}"))?;
    let resp = client
        .post(format!("{}/oneshot", crate::ai::agent::agentd_url()))
        .json(&json!({
            "system": SYSTEM,
            "prompt": format!("Statement text:\n\n{text}"),
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
    let out = body["result"].as_str().unwrap_or("").to_string();

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
