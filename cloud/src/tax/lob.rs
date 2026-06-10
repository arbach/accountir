//! Minimal Lob print-and-mail client (letters API). Auth is HTTP Basic with
//! the API key as username. Configure LOB_API_KEY (test_ keys produce no real
//! mail — ideal until the filing is real).

use reqwest::multipart;
use serde_json::Value;

pub fn api_key() -> Option<String> {
    std::env::var("LOB_API_KEY").ok().filter(|s| !s.is_empty())
}

pub fn configured() -> bool {
    api_key().is_some()
}

fn addr_part(form: multipart::Form, prefix: &str, addr: &Value) -> multipart::Form {
    let mut f = form;
    for (k, lob_k) in [
        ("name", "name"),
        ("address_line1", "address_line1"),
        ("address_line2", "address_line2"),
        ("address_city", "address_city"),
        ("address_state", "address_state"),
        ("address_zip", "address_zip"),
    ] {
        if let Some(v) = addr.get(k).and_then(|v| v.as_str()).filter(|s| !s.is_empty()) {
            f = f.text(format!("{prefix}[{lob_k}]"), v.to_string());
        }
    }
    f
}

/// Create a Lob letter from a PDF. `to`/`from` are objects with name,
/// address_line1, address_line2, address_city, address_state, address_zip.
pub async fn send_letter(
    pdf: &[u8],
    to: &Value,
    from: &Value,
    description: &str,
    certified: bool,
) -> Result<Value, String> {
    let key = api_key().ok_or_else(|| {
        "Lob is not configured: set LOB_API_KEY in /etc/accountir-cloud/env (use a test_ key first)"
            .to_string()
    })?;

    let mut form = multipart::Form::new()
        .text("description", description.to_string())
        .text("color", "false")
        .text("use_type", "operational")
        .part(
            "file",
            multipart::Part::bytes(pdf.to_vec())
                .file_name("document.pdf")
                .mime_str("application/pdf")
                .map_err(|e| format!("mime: {e}"))?,
        );
    if certified {
        form = form.text("extra_service", "certified");
    }
    form = addr_part(form, "to", to);
    form = addr_part(form, "from", from);

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| format!("http client: {e}"))?;
    let resp = client
        .post("https://api.lob.com/v1/letters")
        .basic_auth(&key, Option::<&str>::None)
        .multipart(form)
        .send()
        .await
        .map_err(|e| format!("lob request failed: {e}"))?;
    let status = resp.status();
    let body: Value = resp
        .json()
        .await
        .map_err(|e| format!("lob bad response: {e}"))?;
    if !status.is_success() {
        let msg = body
            .pointer("/error/message")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown error");
        return Err(format!("lob {status}: {msg}"));
    }
    Ok(body)
}
