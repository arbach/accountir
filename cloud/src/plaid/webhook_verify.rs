//! Verification of inbound Plaid webhooks.
//!
//! Plaid signs every webhook with a JWS (ES256) in the `Plaid-Verification`
//! header. The JWS body carries a `request_body_sha256` claim binding it to the
//! exact request body, plus an `iat`. We:
//!   1. parse the JWS header and require alg = ES256,
//!   2. fetch the public JWK for its `kid` from Plaid (cached by kid),
//!   3. verify the JWS signature,
//!   4. require `iat` to be recent (replay window),
//!   5. require sha256(body) to equal the signed `request_body_sha256`.
//!
//! Without this, `/plaid/webhook` (which is intentionally SSO-bypassed at the
//! edge) would accept forged, attacker-controlled payloads.

use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, OnceLock};

use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};
use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

use super::client::PlaidClient;

/// Reject webhooks whose signature is older than this (replay protection).
const MAX_AGE_SECS: i64 = 5 * 60;

#[derive(Debug, thiserror::Error)]
pub enum VerifyError {
    #[error("missing Plaid-Verification header")]
    MissingHeader,
    #[error("malformed JWS: {0}")]
    MalformedJws(String),
    #[error("unexpected JWS algorithm (only ES256 accepted)")]
    BadAlgorithm,
    #[error("could not fetch verification key: {0}")]
    KeyFetch(String),
    #[error("verification key is not a usable P-256 JWK")]
    BadKey,
    #[error("signature verification failed: {0}")]
    BadSignature(String),
    #[error("webhook signature is stale or has a future timestamp")]
    Stale,
    #[error("request body does not match signed hash")]
    BodyMismatch,
}

#[derive(Debug, Deserialize)]
struct Claims {
    iat: i64,
    request_body_sha256: String,
}

/// Process-wide cache of Plaid JWKs keyed by `kid`. Plaid keys are stable and
/// rotate rarely; caching bounds outbound calls to one per distinct kid and
/// removes the per-request amplification a flood of forged webhooks would cause.
fn key_cache() -> &'static Mutex<HashMap<String, Value>> {
    static CACHE: OnceLock<Mutex<HashMap<String, Value>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

async fn jwk_for_kid(client: &PlaidClient, kid: &str) -> Result<Value, VerifyError> {
    if let Some(jwk) = key_cache().lock().unwrap().get(kid).cloned() {
        return Ok(jwk);
    }
    let jwk = client
        .webhook_verification_key_get(kid)
        .await
        .map_err(|e| VerifyError::KeyFetch(e.to_string()))?;
    key_cache()
        .lock()
        .unwrap()
        .insert(kid.to_string(), jwk.clone());
    Ok(jwk)
}

/// Verify the `Plaid-Verification` JWS over `body`. Returns `Ok(())` only when
/// the signature, freshness, and body hash all check out.
pub async fn verify(
    client: &PlaidClient,
    verification_header: Option<&str>,
    body: &[u8],
) -> Result<(), VerifyError> {
    let jws = verification_header.ok_or(VerifyError::MissingHeader)?;

    // 1. Header: require ES256 and pull the key id.
    let header = decode_header(jws).map_err(|e| VerifyError::MalformedJws(e.to_string()))?;
    if header.alg != Algorithm::ES256 {
        return Err(VerifyError::BadAlgorithm);
    }
    let kid = header.kid.ok_or(VerifyError::MalformedJws("no kid".into()))?;

    // 2. Public key for this kid.
    let jwk = jwk_for_kid(client, &kid).await?;
    let x = jwk.get("x").and_then(|v| v.as_str()).ok_or(VerifyError::BadKey)?;
    let y = jwk.get("y").and_then(|v| v.as_str()).ok_or(VerifyError::BadKey)?;
    let key = DecodingKey::from_ec_components(x, y).map_err(|_| VerifyError::BadKey)?;

    // 3. Verify the signature. Plaid's JWS has no exp/aud, only iat + the hash.
    let mut validation = Validation::new(Algorithm::ES256);
    validation.required_spec_claims = HashSet::new();
    validation.validate_exp = false;
    validation.validate_aud = false;
    let claims = decode::<Claims>(jws, &key, &validation)
        .map_err(|e| VerifyError::BadSignature(e.to_string()))?
        .claims;

    // 4. Freshness / replay window (allow a little clock skew either way).
    let now = chrono::Utc::now().timestamp();
    if (now - claims.iat).abs() > MAX_AGE_SECS {
        return Err(VerifyError::Stale);
    }

    // 5. Bind the signed token to this exact body.
    let digest = hex::encode(Sha256::digest(body));
    if !digest.eq_ignore_ascii_case(claims.request_body_sha256.trim()) {
        return Err(VerifyError::BodyMismatch);
    }

    Ok(())
}
