use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::tui::theme::ThemePreset;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub plaid: PlaidConfig,
    #[serde(default)]
    pub crypto: CryptoConfig,
    #[serde(default)]
    pub theme: ThemePreset,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PlaidConfig {
    pub proxy_url: Option<String>,
    pub api_key: Option<String>,
}

impl PlaidConfig {
    pub fn is_configured(&self) -> bool {
        self.proxy_url.is_some() && self.api_key.is_some()
    }
}

/// Configuration for on-chain crypto fetching/verification.
///
/// `explorer_base_url` + `explorer_api_key` drive the primary Etherscan-style backend
/// (Etherscan V2 multichain by default). `alchemy_api_key` is the JSON-RPC fallback used
/// when the explorer is unreachable; if unset it falls back to the `ALCHEMY_API_KEY`
/// environment variable (see [`CryptoConfig::resolved_alchemy_key`]).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CryptoConfig {
    pub explorer_base_url: Option<String>,
    pub explorer_api_key: Option<String>,
    pub alchemy_api_key: Option<String>,
}

impl Default for CryptoConfig {
    fn default() -> Self {
        Self {
            explorer_base_url: Some("https://api.etherscan.io/v2/api".to_string()),
            explorer_api_key: None,
            alchemy_api_key: None,
        }
    }
}

impl CryptoConfig {
    /// True if at least one backend (explorer or Alchemy fallback) can be used.
    pub fn is_configured(&self) -> bool {
        (self.explorer_base_url.is_some() && self.explorer_api_key.is_some())
            || self.resolved_alchemy_key().is_some()
    }

    /// The Alchemy key from config, or the `ALCHEMY_API_KEY` environment variable.
    pub fn resolved_alchemy_key(&self) -> Option<String> {
        self.alchemy_api_key
            .clone()
            .filter(|k| !k.is_empty())
            .or_else(|| std::env::var("ALCHEMY_API_KEY").ok().filter(|k| !k.is_empty()))
    }
}

impl AppConfig {
    pub fn load() -> Self {
        let path = config_path();
        match std::fs::read_to_string(&path) {
            Ok(contents) => toml::from_str(&contents).unwrap_or_default(),
            Err(_) => AppConfig::default(),
        }
    }

    pub fn save(&self) -> anyhow::Result<()> {
        let path = config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let contents = toml::to_string_pretty(self)?;
        std::fs::write(&path, contents)?;
        Ok(())
    }
}

fn config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("accountir")
        .join("config.toml")
}
