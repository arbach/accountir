//! On-chain transaction fetching and verification for crypto wallets.
//!
//! The [`explorer`] module talks to block explorers (Etherscan-style) with an Alchemy
//! JSON-RPC fallback, exposing a small async API used by the CLI/TUI to fetch a wallet's
//! transactions and to re-fetch a single transaction for live verification.

pub mod explorer;

pub use explorer::{ChainSpec, CryptoExplorer, OnChainTx, RawCryptoTx, LEDGER_CRYPTO_DECIMALS};
