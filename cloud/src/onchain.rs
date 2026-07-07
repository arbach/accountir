//! On-chain wallet scanner. Fetches a wallet's ERC-20 + native transfers from
//! Alchemy, stripped of address-poisoning / airdrop spam, so the ledger can be
//! BUILT from a wallet's real crypto activity (assign wallet → pull its tx →
//! classify via the address book → post). This is the write-side counterpart to
//! the read-only `accountir-recon` auditor.
//!
//! NOTE: distinct from `plaid::crypto`, which is AES encryption, not blockchain.

use serde::Deserialize;
use serde_json::json;

/// Direction of a transfer relative to the scanned wallet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    In,
    Out,
}

/// One on-chain transfer, spam-filtered and normalized.
#[derive(Debug, Clone)]
pub struct Transfer {
    pub chain: String,
    pub tx_hash: String,
    pub direction: Direction,
    pub symbol: String,
    /// Amount in the asset's own units (USDC/USDT ≈ 1 USD).
    pub amount: f64,
    pub from: String,
    pub to: String,
    /// ISO-8601 block timestamp (present when withMetadata succeeds).
    pub timestamp: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum ScanError {
    #[error("ALCHEMY_API_KEY not set")]
    NoKey,
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
}

/// The Alchemy API key, from the environment. `None` if unset/empty.
pub fn alchemy_key() -> Option<String> {
    std::env::var("ALCHEMY_API_KEY")
        .ok()
        .filter(|s| !s.is_empty())
}

/// Chains we support via Alchemy (BSC is not on the standard plan — use the
/// recon/Moralis path for that). Accepts common aliases.
fn alchemy_network(chain: &str) -> Option<&'static str> {
    match chain.to_ascii_lowercase().as_str() {
        "eth" | "ethereum" | "mainnet" => Some("eth-mainnet"),
        "polygon" | "matic" => Some("polygon-mainnet"),
        "arbitrum" | "arb" => Some("arb-mainnet"),
        "base" => Some("base-mainnet"),
        "optimism" | "op" => Some("opt-mainnet"),
        _ => None,
    }
}

/// Default scan set — the chains this owner actually transacts on. BSC is where
/// most of the contractor-payment activity lives, served via Moralis.
pub const DEFAULT_CHAINS: &[&str] = &["ethereum", "base", "arbitrum", "polygon", "bsc"];

/// Token tickers we always accept as real value.
const KNOWN_TOKENS: &[&str] = &[
    "USDC", "USDT", "DAI", "USDBC", "USDT0", "PYUSD", "TUSD", "USDE", "USDS", "FDUSD",
    "WETH", "ETH", "WBTC", "CBBTC", "MATIC", "POL", "WMATIC", "STETH", "WSTETH",
];

/// True if a token symbol looks like address-poisoning / airdrop spam.
/// Real tickers are short, alphanumeric, no URLs / punctuation / emoji / ad copy.
/// Those two wallets returned hundreds of "Visit X.com to claim reward" tokens —
/// without this filter the ledger fills with junk.
pub fn is_spam_symbol(symbol: &str) -> bool {
    let s = symbol.trim();
    if s.is_empty() || s.len() > 12 {
        return true;
    }
    if KNOWN_TOKENS.iter().any(|t| t.eq_ignore_ascii_case(s)) {
        return false;
    }
    // Any non-ascii or punctuation beyond a single '.' in a ticker → spam.
    if s.chars().any(|c| !(c.is_ascii_alphanumeric() || c == '.')) {
        return true;
    }
    let upper = s.to_ascii_uppercase();
    // Reject stablecoin look-alikes (address poisoning): normalize digit/letter
    // swaps; if the normalized form is a known token but the raw wasn't, it's fake
    // (e.g. "U5DT" -> "USDT", "USDС" cyrillic already caught above).
    let normalized: String = upper
        .chars()
        .map(|c| match c {
            '0' => 'O',
            '1' => 'I',
            '5' => 'S',
            '8' => 'B',
            _ => c,
        })
        .collect();
    if normalized != upper && KNOWN_TOKENS.iter().any(|t| t.eq_ignore_ascii_case(&normalized)) {
        return true;
    }
    const BAD: &[&str] = &[
        "CLAIM", "VISIT", "REWARD", "ACCESS", "AIRDROP", "BONUS", "REDEEM", "GIFT",
        "EVENT", "POOL", "HTTP", "WWW", ".COM", ".XYZ", ".VIP", ".NET", ".ORG",
        ".LOL", ".APP", "T.LY", "T.ME", "SWAP", "VOUCHER", "COUPON",
    ];
    BAD.iter().any(|b| upper.contains(b))
}

/// Fetch and spam-filter all transfers (both directions) for `address` across
/// the given `chains`. Alchemy for EVM mainnets; Moralis for BSC. A chain that
/// errors is skipped, not fatal.
pub async fn scan_wallet(
    client: &reqwest::Client,
    address: &str,
    chains: &[&str],
) -> Result<Vec<Transfer>, ScanError> {
    let mut out = Vec::new();
    for &chain in chains {
        let mut got = if chain.eq_ignore_ascii_case("bsc") {
            moralis_transfers(client, address, "bsc").await?
        } else if alchemy_network(chain).is_some() {
            alchemy_transfers(client, address, chain).await?
        } else {
            continue;
        };
        out.append(&mut got);
    }
    Ok(out)
}

/// Alchemy path — EVM mainnets (eth/base/arbitrum/polygon/optimism).
async fn alchemy_transfers(
    client: &reqwest::Client,
    address: &str,
    chain: &str,
) -> Result<Vec<Transfer>, ScanError> {
    let key = alchemy_key().ok_or(ScanError::NoKey)?;
    let Some(net) = alchemy_network(chain) else { return Ok(Vec::new()) };
    let url = format!("https://{net}.g.alchemy.com/v2/{key}");
    let mut out = Vec::new();
    for (dir, field) in [(Direction::Out, "fromAddress"), (Direction::In, "toAddress")] {
        let body = json!({
            "jsonrpc": "2.0", "id": 1, "method": "alchemy_getAssetTransfers",
            "params": [{
                field: address,
                "category": ["erc20", "external"],
                "withMetadata": true,
                "excludeZeroValue": true,
                "maxCount": "0x3e8",
                "order": "asc"
            }]
        });
        let resp = client.post(&url).json(&body).send().await?;
        let parsed: AlchemyResp = match resp.json().await {
            Ok(v) => v,
            Err(_) => continue,
        };
        let Some(result) = parsed.result else { continue };
        for t in result.transfers {
            let symbol = t.asset.unwrap_or_default();
            if is_spam_symbol(&symbol) {
                continue;
            }
            out.push(Transfer {
                chain: chain.to_string(),
                tx_hash: t.hash.unwrap_or_default(),
                direction: dir,
                symbol,
                amount: t.value.unwrap_or(0.0),
                from: t.from.unwrap_or_default(),
                to: t.to.unwrap_or_default(),
                timestamp: t.metadata.and_then(|m| m.block_timestamp),
            });
        }
    }
    Ok(out)
}

/// The Moralis API key, from the environment.
pub fn moralis_key() -> Option<String> {
    std::env::var("MORALIS_API_KEY")
        .ok()
        .filter(|s| !s.is_empty())
}

/// Moralis path — BSC (not served by Alchemy's plan).
async fn moralis_transfers(
    client: &reqwest::Client,
    address: &str,
    chain: &str,
) -> Result<Vec<Transfer>, ScanError> {
    let Some(key) = moralis_key() else { return Ok(Vec::new()) };
    let url = format!(
        "https://deep-index.moralis.io/api/v2.2/{address}/erc20/transfers?chain={chain}&limit=100"
    );
    let resp = client
        .get(&url)
        .header("X-API-Key", key)
        .header("accept", "application/json")
        .send()
        .await?;
    let parsed: MoralisResp = match resp.json().await {
        Ok(v) => v,
        Err(_) => return Ok(Vec::new()),
    };
    let a = address.to_lowercase();
    let mut out = Vec::new();
    for r in parsed.result {
        let symbol = r.token_symbol.unwrap_or_default();
        if is_spam_symbol(&symbol) {
            continue;
        }
        let is_out = r.from_address.as_deref().map(str::to_lowercase).as_deref() == Some(a.as_str());
        out.push(Transfer {
            chain: chain.to_string(),
            tx_hash: r.transaction_hash.unwrap_or_default(),
            direction: if is_out { Direction::Out } else { Direction::In },
            symbol,
            amount: r
                .value_decimal
                .as_deref()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.0),
            from: r.from_address.unwrap_or_default(),
            to: r.to_address.unwrap_or_default(),
            timestamp: r.block_timestamp,
        });
    }
    Ok(out)
}

#[derive(Deserialize)]
struct AlchemyResp {
    result: Option<AlchemyResult>,
}
#[derive(Deserialize)]
struct AlchemyResult {
    #[serde(default)]
    transfers: Vec<AlchemyTransfer>,
}
#[derive(Deserialize)]
struct AlchemyTransfer {
    hash: Option<String>,
    from: Option<String>,
    to: Option<String>,
    asset: Option<String>,
    value: Option<f64>,
    #[serde(default)]
    metadata: Option<AlchemyMeta>,
}
#[derive(Deserialize)]
struct AlchemyMeta {
    #[serde(rename = "blockTimestamp")]
    block_timestamp: Option<String>,
}

#[derive(Deserialize)]
struct MoralisResp {
    #[serde(default)]
    result: Vec<MoralisTransfer>,
}
#[derive(Deserialize)]
struct MoralisTransfer {
    transaction_hash: Option<String>,
    from_address: Option<String>,
    to_address: Option<String>,
    token_symbol: Option<String>,
    value_decimal: Option<String>,
    block_timestamp: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spam_filter_keeps_real_tokens_rejects_junk() {
        assert!(!is_spam_symbol("USDC"));
        assert!(!is_spam_symbol("usdt"));
        assert!(!is_spam_symbol("ETH"));
        assert!(!is_spam_symbol("WBTC"));
        assert!(is_spam_symbol("Visit rocketpool.win to claim reward"));
        assert!(is_spam_symbol("$ Check: blastcode.io Your AirDrop Code"));
        assert!(is_spam_symbol("ACCESS [ETHNA.CC] TO CLAIM"));
        assert!(is_spam_symbol("UЅDС")); // cyrillic-lookalike poisoning
        assert!(is_spam_symbol("U5DT")); // digit look-alike
        assert!(is_spam_symbol(""));
    }
}
