//! Commands for importing and verifying on-chain crypto transactions.
//!
//! Network access (fetching tx lists / single txs) happens in the async CLI/TUI layer via
//! [`crate::crypto::CryptoExplorer`]; this module is the synchronous DB side that dedups,
//! posts journal entries, and records the `crypto_transactions` provenance rows.

use chrono::Utc;
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

use crate::commands::account_commands::find_or_create_uncategorized;
use crate::commands::entry_commands::{EntryCommands, EntryLine, PostEntryCommand};
use crate::crypto::{CryptoExplorer, OnChainTx, RawCryptoTx, LEDGER_CRYPTO_DECIMALS};
use crate::events::types::{Event, JournalEntrySource};
use crate::store::event_store::EventStore;

#[derive(Error, Debug)]
pub enum CryptoCommandError {
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("Account error: {0}")]
    Account(#[from] crate::commands::account_commands::AccountCommandError),
    #[error("Wallet not found: {0}")]
    WalletNotFound(String),
}

/// A connected crypto wallet (on-chain address mapped to a ledger account).
#[derive(Debug, Clone)]
pub struct CryptoWallet {
    pub id: String,
    pub chain: String,
    pub address: String,
    pub local_account_id: String,
    pub label: Option<String>,
    pub explorer_base_url: Option<String>,
}

/// Summary of an import run.
#[derive(Debug, Clone, Default)]
pub struct ImportSummary {
    pub imported: usize,
    pub skipped_duplicate: usize,
    pub skipped_invalid: usize,
}

/// Summary of a verification run.
#[derive(Debug, Clone, Default)]
pub struct VerifySummary {
    pub verified: usize,
    pub failed: usize,
}

/// Buffered provenance row, inserted after the entry-posting borrow is released.
struct PendingCryptoRow {
    chain: String,
    tx_hash: String,
    entry_id: String,
    wallet_id: String,
    address: String,
    amount: i64,
    asset: String,
    block_number: Option<i64>,
    block_time: Option<String>,
    explorer_url: String,
    content_hash: String,
}

/// Canonical content hash for an on-chain transaction (used to detect tampering).
pub fn content_hash(
    chain: &str,
    tx_hash: &str,
    from: &str,
    to: &str,
    value_raw: &str,
    block_number: Option<i64>,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(chain.as_bytes());
    hasher.update(b"|");
    hasher.update(tx_hash.to_lowercase().as_bytes());
    hasher.update(b"|");
    hasher.update(from.to_lowercase().as_bytes());
    hasher.update(b"|");
    hasher.update(to.to_lowercase().as_bytes());
    hasher.update(b"|");
    hasher.update(value_raw.as_bytes());
    hasher.update(b"|");
    hasher.update(block_number.unwrap_or(-1).to_string().as_bytes());
    hex::encode(hasher.finalize())
}

pub struct CryptoCommands<'a> {
    store: &'a mut EventStore,
    user_id: String,
}

impl<'a> CryptoCommands<'a> {
    pub fn new(store: &'a mut EventStore, user_id: String) -> Self {
        Self { store, user_id }
    }

    /// Connect (or update) a wallet mapping an on-chain address to a ledger account.
    /// Returns the wallet id.
    pub fn connect_wallet(
        &mut self,
        chain: &str,
        address: &str,
        local_account_id: &str,
        label: Option<&str>,
        explorer_base_url: Option<&str>,
    ) -> Result<String, CryptoCommandError> {
        let conn = self.store.connection();
        // Reuse the existing id if this (chain, address) is already connected.
        let existing: Option<String> = conn
            .query_row(
                "SELECT id FROM crypto_wallets WHERE chain = ?1 AND address = ?2",
                rusqlite::params![chain, address],
                |row| row.get(0),
            )
            .ok();
        let id = existing.unwrap_or_else(|| Uuid::new_v4().to_string());
        conn.execute(
            "INSERT INTO crypto_wallets (id, chain, address, local_account_id, label, explorer_base_url)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(chain, address) DO UPDATE SET
                local_account_id = excluded.local_account_id,
                label = excluded.label,
                explorer_base_url = excluded.explorer_base_url",
            rusqlite::params![id, chain, address, local_account_id, label, explorer_base_url],
        )?;
        Ok(id)
    }

    /// List all connected wallets.
    pub fn list_wallets(&self) -> Result<Vec<CryptoWallet>, CryptoCommandError> {
        let conn = self.store.connection();
        let mut stmt = conn.prepare(
            "SELECT id, chain, address, local_account_id, label, explorer_base_url
             FROM crypto_wallets ORDER BY created_at",
        )?;
        let rows = stmt
            .query_map([], |row| {
                Ok(CryptoWallet {
                    id: row.get(0)?,
                    chain: row.get(1)?,
                    address: row.get(2)?,
                    local_account_id: row.get(3)?,
                    label: row.get(4)?,
                    explorer_base_url: row.get(5)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(rows)
    }

    /// Load a single wallet by id.
    pub fn get_wallet(&self, wallet_id: &str) -> Result<CryptoWallet, CryptoCommandError> {
        self.store
            .connection()
            .query_row(
                "SELECT id, chain, address, local_account_id, label, explorer_base_url
                 FROM crypto_wallets WHERE id = ?1",
                [wallet_id],
                |row| {
                    Ok(CryptoWallet {
                        id: row.get(0)?,
                        chain: row.get(1)?,
                        address: row.get(2)?,
                        local_account_id: row.get(3)?,
                        label: row.get(4)?,
                        explorer_base_url: row.get(5)?,
                    })
                },
            )
            .map_err(|_| CryptoCommandError::WalletNotFound(wallet_id.to_string()))
    }

    /// Import fetched transactions for a wallet: dedup, post a balanced journal entry per
    /// new tx, and record the provenance row (hash + explorer link, unverified).
    pub fn import_transactions(
        &mut self,
        wallet: &CryptoWallet,
        txs: &[RawCryptoTx],
        explorer: &CryptoExplorer,
    ) -> Result<ImportSummary, CryptoCommandError> {
        let uncategorized_id = find_or_create_uncategorized(self.store)?;

        // Pre-load already-imported tx hashes for this chain so we can dedup without
        // re-borrowing the store inside the entry-posting loop (mirrors the Plaid path).
        let mut seen: std::collections::HashSet<String> = {
            let conn = self.store.connection();
            let mut stmt =
                conn.prepare("SELECT tx_hash FROM crypto_transactions WHERE chain = ?1")?;
            let set: std::collections::HashSet<String> = stmt
                .query_map([&wallet.chain], |row| row.get::<_, String>(0))?
                .filter_map(|r| r.ok())
                .collect();
            set
        };

        let mut summary = ImportSummary::default();
        let mut pending: Vec<PendingCryptoRow> = Vec::new();

        let mut commands = EntryCommands::new(self.store, self.user_id.clone());

        for tx in txs {
            if tx.is_error {
                summary.skipped_invalid += 1;
                continue;
            }
            let amount = match tx.ledger_amount() {
                Some(a) if a != 0 => a,
                _ => {
                    summary.skipped_invalid += 1;
                    continue;
                }
            };

            // Dedup against already-imported txs (and earlier txs in this same batch).
            if seen.contains(&tx.tx_hash) {
                summary.skipped_duplicate += 1;
                continue;
            }
            seen.insert(tx.tx_hash.clone());

            let date = tx
                .time_stamp
                .and_then(|ts| chrono::DateTime::from_timestamp(ts, 0))
                .map(|dt| dt.date_naive())
                .unwrap_or_else(|| Utc::now().date_naive());

            let lines = vec![
                EntryLine {
                    account_id: wallet.local_account_id.clone(),
                    amount,
                    currency: tx.asset.clone(),
                    exchange_rate: None,
                    memo: None,
                },
                EntryLine {
                    account_id: uncategorized_id.clone(),
                    amount: -amount,
                    currency: tx.asset.clone(),
                    exchange_rate: None,
                    memo: None,
                },
            ];

            let memo = format!(
                "{} {} {}",
                if tx.direction_in { "Received" } else { "Sent" },
                tx.asset,
                short_hash(&tx.tx_hash)
            );

            match commands.post_entry(PostEntryCommand {
                date,
                memo,
                lines,
                reference: Some(tx.tx_hash.clone()),
                source: Some(JournalEntrySource::Crypto),
            }) {
                Ok(stored) => {
                    let entry_id = match &stored.event {
                        Event::JournalEntryPosted { entry_id, .. } => entry_id.clone(),
                        _ => continue,
                    };
                    pending.push(PendingCryptoRow {
                        chain: wallet.chain.clone(),
                        tx_hash: tx.tx_hash.clone(),
                        entry_id,
                        wallet_id: wallet.id.clone(),
                        address: wallet.address.clone(),
                        amount,
                        asset: tx.asset.clone(),
                        block_number: tx.block_number,
                        block_time: tx.time_stamp.and_then(|ts| {
                            chrono::DateTime::from_timestamp(ts, 0).map(|dt| dt.to_rfc3339())
                        }),
                        explorer_url: explorer.tx_url(&tx.tx_hash),
                        content_hash: content_hash(
                            &wallet.chain,
                            &tx.tx_hash,
                            &tx.from,
                            &tx.to,
                            &tx.value_raw,
                            tx.block_number,
                        ),
                    });
                    summary.imported += 1;
                }
                Err(e) => {
                    eprintln!("Failed to import crypto tx {}: {}", tx.tx_hash, e);
                    summary.skipped_invalid += 1;
                }
            }
        }

        drop(commands);

        let conn = self.store.connection();
        for row in pending {
            conn.execute(
                "INSERT OR IGNORE INTO crypto_transactions
                    (chain, tx_hash, entry_id, wallet_id, address, amount, asset,
                     block_number, block_time, explorer_url, content_hash, verified)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, 0)",
                rusqlite::params![
                    row.chain,
                    row.tx_hash,
                    row.entry_id,
                    row.wallet_id,
                    row.address,
                    row.amount,
                    row.asset,
                    row.block_number,
                    row.block_time,
                    row.explorer_url,
                    row.content_hash,
                ],
            )?;
        }

        Ok(summary)
    }

    /// All imported tx hashes for a wallet, for the verification pass.
    pub fn wallet_tx_hashes(&self, wallet_id: &str) -> Result<Vec<String>, CryptoCommandError> {
        let conn = self.store.connection();
        let mut stmt =
            conn.prepare("SELECT tx_hash FROM crypto_transactions WHERE wallet_id = ?1")?;
        let rows = stmt
            .query_map([wallet_id], |row| row.get::<_, String>(0))?
            .filter_map(|r| r.ok())
            .collect();
        Ok(rows)
    }

    /// Live-verify one transaction against freshly fetched on-chain data: confirm it
    /// executed successfully, the block matches, and (for native transfers) the amount
    /// matches. Records the verified flag / error on the provenance row.
    pub fn record_verification(
        &mut self,
        chain: &str,
        tx_hash: &str,
        onchain: Option<&OnChainTx>,
    ) -> Result<bool, CryptoCommandError> {
        let stored: Option<(i64, Option<i64>, String)> = self
            .store
            .connection()
            .query_row(
                "SELECT amount, block_number, asset FROM crypto_transactions
                 WHERE chain = ?1 AND tx_hash = ?2",
                rusqlite::params![chain, tx_hash],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .ok();

        let (stored_amount, stored_block, stored_asset) = match stored {
            Some(v) => v,
            None => return Ok(false),
        };

        let error: Option<String> = match onchain {
            None => Some("transaction not found on-chain".to_string()),
            Some(tx) if !tx.success => Some("transaction failed on-chain".to_string()),
            Some(tx) => {
                let mut err = None;
                if let (Some(a), Some(b)) = (stored_block, tx.block_number) {
                    if a != b {
                        err = Some(format!("block mismatch: stored {a}, on-chain {b}"));
                    }
                }
                // Amount check only applies to the chain's native asset (token transfers
                // carry a zero native value on the parent tx).
                if err.is_none() && stored_asset.eq_ignore_ascii_case(chain) {
                    if let Some(onchain_amount) =
                        crate::crypto::explorer::scale_raw_value(&tx.value_raw, 18)
                    {
                        if onchain_amount != stored_amount.abs() {
                            err = Some(format!(
                                "amount mismatch: stored {}, on-chain {}",
                                stored_amount.abs(),
                                onchain_amount
                            ));
                        }
                    }
                }
                err
            }
        };

        let conn = self.store.connection();
        match error {
            None => {
                conn.execute(
                    "UPDATE crypto_transactions
                     SET verified = 1, verified_at = ?1, verify_error = NULL
                     WHERE chain = ?2 AND tx_hash = ?3",
                    rusqlite::params![Utc::now().to_rfc3339(), chain, tx_hash],
                )?;
                Ok(true)
            }
            Some(msg) => {
                conn.execute(
                    "UPDATE crypto_transactions
                     SET verified = 0, verified_at = ?1, verify_error = ?2
                     WHERE chain = ?3 AND tx_hash = ?4",
                    rusqlite::params![Utc::now().to_rfc3339(), msg, chain, tx_hash],
                )?;
                Ok(false)
            }
        }
    }
}

fn short_hash(hash: &str) -> String {
    if hash.len() > 12 {
        format!("{}…{}", &hash[..8], &hash[hash.len() - 4..])
    } else {
        hash.to_string()
    }
}

/// The decimal scale crypto ledger amounts use (re-exported for callers/tests).
pub const CRYPTO_DECIMALS: u32 = LEDGER_CRYPTO_DECIMALS;
