//! Block-explorer client for fetching and verifying on-chain transactions.
//!
//! Primary backend: Etherscan V2 multichain API (one API key, `chainid` selects the chain).
//! Fallback backend: Alchemy JSON-RPC (`ALCHEMY_API_KEY`) — used for single-tx verification
//! when the explorer is unreachable, and for listing via `alchemy_getAssetTransfers`.

use anyhow::{anyhow, Context, Result};
use serde_json::Value;

/// Number of decimal places crypto amounts are scaled to before being stored as an
/// integer in the ledger. Raw on-chain values (often 18 decimals / wei) overflow an i64,
/// so we down-scale to a fixed accounting precision.
pub const LEDGER_CRYPTO_DECIMALS: u32 = 8;

/// Static per-chain configuration.
#[derive(Debug, Clone)]
pub struct ChainSpec {
    pub name: &'static str,
    pub chain_id: u64,
    /// Human explorer base for building clickable tx links, e.g. `https://etherscan.io`.
    pub explorer_tx_base: &'static str,
    /// Alchemy network slug for the JSON-RPC fallback, e.g. `eth-mainnet`.
    pub alchemy_network: &'static str,
}

/// Look up a known EVM chain by name (case-insensitive).
pub fn chain_spec(name: &str) -> Option<ChainSpec> {
    let n = name.trim().to_lowercase();
    let spec = match n.as_str() {
        "ethereum" | "eth" | "mainnet" => ChainSpec {
            name: "ethereum",
            chain_id: 1,
            explorer_tx_base: "https://etherscan.io",
            alchemy_network: "eth-mainnet",
        },
        "polygon" | "matic" => ChainSpec {
            name: "polygon",
            chain_id: 137,
            explorer_tx_base: "https://polygonscan.com",
            alchemy_network: "polygon-mainnet",
        },
        "arbitrum" | "arb" => ChainSpec {
            name: "arbitrum",
            chain_id: 42161,
            explorer_tx_base: "https://arbiscan.io",
            alchemy_network: "arb-mainnet",
        },
        "optimism" | "op" => ChainSpec {
            name: "optimism",
            chain_id: 10,
            explorer_tx_base: "https://optimistic.etherscan.io",
            alchemy_network: "opt-mainnet",
        },
        "base" => ChainSpec {
            name: "base",
            chain_id: 8453,
            explorer_tx_base: "https://basescan.org",
            alchemy_network: "base-mainnet",
        },
        _ => return None,
    };
    Some(spec)
}

/// A transaction as fetched from a listing endpoint, normalized relative to a wallet address.
#[derive(Debug, Clone)]
pub struct RawCryptoTx {
    pub tx_hash: String,
    pub from: String,
    pub to: String,
    /// Raw value in the asset's smallest unit (decimal string).
    pub value_raw: String,
    pub asset: String,
    pub decimals: u32,
    pub block_number: Option<i64>,
    /// Unix seconds, if known.
    pub time_stamp: Option<i64>,
    /// Whether the chain reports the tx as failed.
    pub is_error: bool,
    /// True if value flowed INTO the queried address (a receive).
    pub direction_in: bool,
}

impl RawCryptoTx {
    /// Signed ledger amount (positive = received/debit asset, negative = sent/credit asset),
    /// scaled to [`LEDGER_CRYPTO_DECIMALS`]. Returns None on parse/overflow.
    pub fn ledger_amount(&self) -> Option<i64> {
        let scaled = scale_raw_value(&self.value_raw, self.decimals)?;
        Some(if self.direction_in { scaled } else { -scaled })
    }
}

/// A transaction re-fetched by hash, used for live verification.
#[derive(Debug, Clone)]
pub struct OnChainTx {
    pub from: String,
    pub to: String,
    /// Raw native value in wei (decimal string).
    pub value_raw: String,
    pub block_number: Option<i64>,
    /// True if the tx executed successfully (receipt status == 1).
    pub success: bool,
}

/// Scale a raw smallest-unit value string (non-negative integer) down to
/// [`LEDGER_CRYPTO_DECIMALS`]. Returns None on parse/overflow.
pub fn scale_raw_value(raw: &str, decimals: u32) -> Option<i64> {
    let raw_i: i128 = raw.trim().parse().ok()?;
    let scaled: i128 = if decimals >= LEDGER_CRYPTO_DECIMALS {
        let div = 10i128.checked_pow(decimals - LEDGER_CRYPTO_DECIMALS)?;
        // Round to nearest (values are non-negative).
        (raw_i + div / 2) / div
    } else {
        let mul = 10i128.checked_pow(LEDGER_CRYPTO_DECIMALS - decimals)?;
        raw_i.checked_mul(mul)?
    };
    i64::try_from(scaled).ok()
}

/// Scale a human-unit value (e.g. 1.5 ETH) to [`LEDGER_CRYPTO_DECIMALS`].
fn scale_human_value(value: f64) -> Option<i64> {
    let multiplier = 10f64.powi(LEDGER_CRYPTO_DECIMALS as i32);
    let scaled = (value * multiplier).round();
    if scaled.is_finite() {
        Some(scaled as i64)
    } else {
        None
    }
}

fn hex_to_i64(s: &str) -> Option<i64> {
    let s = s.trim_start_matches("0x");
    i64::from_str_radix(s, 16).ok()
}

fn hex_to_decimal_string(s: &str) -> String {
    let s = s.trim_start_matches("0x");
    match u128::from_str_radix(s, 16) {
        Ok(v) => v.to_string(),
        Err(_) => "0".to_string(),
    }
}

/// Explorer client for one chain, with an optional Alchemy fallback.
pub struct CryptoExplorer {
    pub chain: ChainSpec,
    /// Etherscan-style API base, e.g. `https://api.etherscan.io/v2/api`.
    explorer_base_url: Option<String>,
    explorer_api_key: Option<String>,
    alchemy_api_key: Option<String>,
    client: reqwest::Client,
}

impl CryptoExplorer {
    pub fn new(
        chain: ChainSpec,
        explorer_base_url: Option<String>,
        explorer_api_key: Option<String>,
        alchemy_api_key: Option<String>,
    ) -> Self {
        Self {
            chain,
            explorer_base_url,
            explorer_api_key,
            alchemy_api_key,
            client: reqwest::Client::new(),
        }
    }

    /// Build a clickable explorer URL for a transaction hash.
    pub fn tx_url(&self, tx_hash: &str) -> String {
        format!("{}/tx/{}", self.chain.explorer_tx_base, tx_hash)
    }

    fn alchemy_url(&self) -> Option<String> {
        self.alchemy_api_key
            .as_ref()
            .map(|k| format!("https://{}.g.alchemy.com/v2/{}", self.chain.alchemy_network, k))
    }

    /// List native + ERC-20 transactions for an address. Tries the Etherscan-style API
    /// first; falls back to Alchemy `getAssetTransfers` if the explorer is unconfigured
    /// or fails.
    pub async fn list_address_txs(&self, address: &str) -> Result<Vec<RawCryptoTx>> {
        if self.explorer_base_url.is_some() && self.explorer_api_key.is_some() {
            match self.list_via_etherscan(address).await {
                Ok(txs) => return Ok(txs),
                Err(e) => {
                    eprintln!("Explorer listing failed ({e}); trying Alchemy fallback…");
                }
            }
        }
        if self.alchemy_url().is_some() {
            return self.list_via_alchemy(address).await;
        }
        Err(anyhow!(
            "No explorer configured and no Alchemy fallback key available"
        ))
    }

    /// Re-fetch a single transaction by hash for verification. Tries the explorer proxy
    /// first, then Alchemy JSON-RPC.
    pub async fn get_tx(&self, tx_hash: &str) -> Result<Option<OnChainTx>> {
        if self.explorer_base_url.is_some() && self.explorer_api_key.is_some() {
            match self.get_tx_via_etherscan(tx_hash).await {
                Ok(Some(tx)) => return Ok(Some(tx)),
                Ok(None) => {}
                Err(e) => eprintln!("Explorer get_tx failed ({e}); trying Alchemy fallback…"),
            }
        }
        if self.alchemy_url().is_some() {
            return self.get_tx_via_alchemy(tx_hash).await;
        }
        Ok(None)
    }

    // ── Etherscan-style backend ──────────────────────────────────────────────

    /// Issue an Etherscan-style GET, building the query string manually (this reqwest
    /// build does not expose `RequestBuilder::query`).
    async fn etherscan_get(&self, params: &[(&str, &str)]) -> Result<Value> {
        let base = self
            .explorer_base_url
            .as_ref()
            .ok_or_else(|| anyhow!("no explorer base url"))?;
        let qs: Vec<String> = params.iter().map(|(k, v)| format!("{k}={v}")).collect();
        let url = format!("{}?{}", base, qs.join("&"));
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .context("etherscan request")?
            .json::<Value>()
            .await
            .context("etherscan decode")?;
        Ok(resp)
    }

    async fn list_via_etherscan(&self, address: &str) -> Result<Vec<RawCryptoTx>> {
        let key = self.explorer_api_key.as_ref().unwrap().clone();
        let chain_id = self.chain.chain_id.to_string();
        let addr_lc = address.to_lowercase();

        let mut out = Vec::new();

        // Native transactions.
        let native = self
            .etherscan_get(&[
                ("chainid", chain_id.as_str()),
                ("module", "account"),
                ("action", "txlist"),
                ("address", address),
                ("sort", "asc"),
                ("apikey", key.as_str()),
            ])
            .await?;
        if let Some(arr) = native.get("result").and_then(|r| r.as_array()) {
            for item in arr {
                let to = item.get("to").and_then(|v| v.as_str()).unwrap_or("");
                out.push(RawCryptoTx {
                    tx_hash: item.get("hash").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    from: item.get("from").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    to: to.to_string(),
                    value_raw: item.get("value").and_then(|v| v.as_str()).unwrap_or("0").to_string(),
                    asset: self.chain.name.to_uppercase(),
                    decimals: 18,
                    block_number: item
                        .get("blockNumber")
                        .and_then(|v| v.as_str())
                        .and_then(|s| s.parse().ok()),
                    time_stamp: item
                        .get("timeStamp")
                        .and_then(|v| v.as_str())
                        .and_then(|s| s.parse().ok()),
                    is_error: item.get("isError").and_then(|v| v.as_str()).unwrap_or("0") == "1",
                    direction_in: to.to_lowercase() == addr_lc,
                });
            }
        }

        // ERC-20 token transfers.
        let tokens = self
            .etherscan_get(&[
                ("chainid", chain_id.as_str()),
                ("module", "account"),
                ("action", "tokentx"),
                ("address", address),
                ("sort", "asc"),
                ("apikey", key.as_str()),
            ])
            .await?;
        if let Some(arr) = tokens.get("result").and_then(|r| r.as_array()) {
            for item in arr {
                let to = item.get("to").and_then(|v| v.as_str()).unwrap_or("");
                out.push(RawCryptoTx {
                    tx_hash: item.get("hash").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    from: item.get("from").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    to: to.to_string(),
                    value_raw: item.get("value").and_then(|v| v.as_str()).unwrap_or("0").to_string(),
                    asset: item
                        .get("tokenSymbol")
                        .and_then(|v| v.as_str())
                        .unwrap_or("TOKEN")
                        .to_string(),
                    decimals: item
                        .get("tokenDecimal")
                        .and_then(|v| v.as_str())
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(18),
                    block_number: item
                        .get("blockNumber")
                        .and_then(|v| v.as_str())
                        .and_then(|s| s.parse().ok()),
                    time_stamp: item
                        .get("timeStamp")
                        .and_then(|v| v.as_str())
                        .and_then(|s| s.parse().ok()),
                    is_error: false,
                    direction_in: to.to_lowercase() == addr_lc,
                });
            }
        }

        Ok(out)
    }

    async fn get_tx_via_etherscan(&self, tx_hash: &str) -> Result<Option<OnChainTx>> {
        let key = self.explorer_api_key.as_ref().unwrap().clone();
        let chain_id = self.chain.chain_id.to_string();

        let tx = self
            .etherscan_get(&[
                ("chainid", chain_id.as_str()),
                ("module", "proxy"),
                ("action", "eth_getTransactionByHash"),
                ("txhash", tx_hash),
                ("apikey", key.as_str()),
            ])
            .await?;

        let result = match tx.get("result") {
            Some(Value::Object(_)) => &tx["result"],
            _ => return Ok(None),
        };

        let receipt = self
            .etherscan_get(&[
                ("chainid", chain_id.as_str()),
                ("module", "proxy"),
                ("action", "eth_getTransactionReceipt"),
                ("txhash", tx_hash),
                ("apikey", key.as_str()),
            ])
            .await?;
        let success = receipt
            .get("result")
            .and_then(|r| r.get("status"))
            .and_then(|s| s.as_str())
            .map(|s| s == "0x1")
            .unwrap_or(true);

        Ok(Some(OnChainTx {
            from: result.get("from").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            to: result.get("to").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            value_raw: result
                .get("value")
                .and_then(|v| v.as_str())
                .map(hex_to_decimal_string)
                .unwrap_or_else(|| "0".to_string()),
            block_number: result
                .get("blockNumber")
                .and_then(|v| v.as_str())
                .and_then(hex_to_i64),
            success,
        }))
    }

    // ── Alchemy JSON-RPC fallback ────────────────────────────────────────────

    async fn rpc(&self, method: &str, params: Value) -> Result<Value> {
        let url = self
            .alchemy_url()
            .ok_or_else(|| anyhow!("no Alchemy key configured"))?;
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": method,
            "params": params,
        });
        let resp = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .with_context(|| format!("alchemy {method} request"))?
            .json::<Value>()
            .await
            .with_context(|| format!("alchemy {method} decode"))?;
        Ok(resp)
    }

    async fn get_tx_via_alchemy(&self, tx_hash: &str) -> Result<Option<OnChainTx>> {
        let tx = self
            .rpc("eth_getTransactionByHash", serde_json::json!([tx_hash]))
            .await?;
        let result = match tx.get("result") {
            Some(Value::Object(_)) => tx["result"].clone(),
            _ => return Ok(None),
        };
        let receipt = self
            .rpc("eth_getTransactionReceipt", serde_json::json!([tx_hash]))
            .await?;
        let success = receipt
            .get("result")
            .and_then(|r| r.get("status"))
            .and_then(|s| s.as_str())
            .map(|s| s == "0x1")
            .unwrap_or(true);

        Ok(Some(OnChainTx {
            from: result.get("from").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            to: result.get("to").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            value_raw: result
                .get("value")
                .and_then(|v| v.as_str())
                .map(hex_to_decimal_string)
                .unwrap_or_else(|| "0".to_string()),
            block_number: result
                .get("blockNumber")
                .and_then(|v| v.as_str())
                .and_then(hex_to_i64),
            success,
        }))
    }

    async fn list_via_alchemy(&self, address: &str) -> Result<Vec<RawCryptoTx>> {
        let addr_lc = address.to_lowercase();
        let mut out = Vec::new();

        // Incoming and outgoing transfers are separate queries in getAssetTransfers.
        for (direction_in, key, val) in [
            (true, "toAddress", address),
            (false, "fromAddress", address),
        ] {
            let params = serde_json::json!([{
                "fromBlock": "0x0",
                "toBlock": "latest",
                key: val,
                "category": ["external", "erc20"],
                "withMetadata": true,
                "excludeZeroValue": false,
                "maxCount": "0x3e8",
            }]);
            let resp = self.rpc("alchemy_getAssetTransfers", params).await?;
            let transfers = resp
                .get("result")
                .and_then(|r| r.get("transfers"))
                .and_then(|t| t.as_array())
                .cloned()
                .unwrap_or_default();
            for t in transfers {
                let to = t.get("to").and_then(|v| v.as_str()).unwrap_or("");
                // Alchemy returns `value` already in human units (a JSON number).
                let human = t.get("value").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let value_raw = scale_human_value(human)
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "0".to_string());
                let block_number = t
                    .get("blockNum")
                    .and_then(|v| v.as_str())
                    .and_then(hex_to_i64);
                out.push(RawCryptoTx {
                    tx_hash: t.get("hash").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    from: t.get("from").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    to: to.to_string(),
                    // Already scaled to LEDGER_CRYPTO_DECIMALS, so report decimals accordingly.
                    value_raw,
                    asset: t
                        .get("asset")
                        .and_then(|v| v.as_str())
                        .unwrap_or(&self.chain.name.to_uppercase())
                        .to_string(),
                    decimals: LEDGER_CRYPTO_DECIMALS,
                    block_number,
                    time_stamp: None,
                    is_error: false,
                    direction_in: if to.to_lowercase() == addr_lc {
                        true
                    } else {
                        direction_in
                    },
                });
            }
        }

        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scale_raw_value_wei() {
        // 1 ETH = 1e18 wei -> scaled to 8dp = 1e8.
        assert_eq!(scale_raw_value("1000000000000000000", 18), Some(100_000_000));
        // 0.5 ETH.
        assert_eq!(scale_raw_value("500000000000000000", 18), Some(50_000_000));
        // 1 USDC (6 decimals) -> 1e8.
        assert_eq!(scale_raw_value("1000000", 6), Some(100_000_000));
    }

    #[test]
    fn test_ledger_amount_direction() {
        let mut tx = RawCryptoTx {
            tx_hash: "0xabc".into(),
            from: "0x1".into(),
            to: "0x2".into(),
            value_raw: "1000000000000000000".into(),
            asset: "ETH".into(),
            decimals: 18,
            block_number: Some(1),
            time_stamp: None,
            is_error: false,
            direction_in: true,
        };
        assert_eq!(tx.ledger_amount(), Some(100_000_000));
        tx.direction_in = false;
        assert_eq!(tx.ledger_amount(), Some(-100_000_000));
    }

    #[test]
    fn test_chain_spec_lookup() {
        assert_eq!(chain_spec("ethereum").unwrap().chain_id, 1);
        assert_eq!(chain_spec("ETH").unwrap().chain_id, 1);
        assert_eq!(chain_spec("base").unwrap().chain_id, 8453);
        assert!(chain_spec("dogecoin").is_none());
    }
}
