use chrono::NaiveDate;
use rusqlite::{params, Connection};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::commands::account_commands::find_or_create_uncategorized;
use crate::commands::entry_commands::{EntryCommands, EntryLine, PostEntryCommand};
use crate::domain::AccountType;
use crate::events::types::{Event, JournalEntrySource};
use crate::store::event_store::EventStore;

// ── CSV/bank file parsing utilities ──────────────────────────────────────────

/// Parse a delimited line, handling quoted fields.
pub fn parse_delimited_line(line: &str, delimiter: char) -> Vec<String> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '"' => {
                if in_quotes {
                    if chars.peek() == Some(&'"') {
                        current.push('"');
                        chars.next();
                    } else {
                        in_quotes = false;
                    }
                } else {
                    in_quotes = true;
                }
            }
            c if c == delimiter && !in_quotes => {
                fields.push(current.trim().to_string());
                current = String::new();
            }
            _ => {
                current.push(c);
            }
        }
    }

    fields.push(current.trim().to_string());
    fields
}

/// Parse a date string in various common formats.
pub fn parse_date(s: &str) -> Option<NaiveDate> {
    let s = s.trim();

    for fmt in &[
        "%Y/%m/%d", "%Y-%m-%d", "%m/%d/%y", "%m-%d-%y", "%m/%d/%Y", "%m-%d-%Y",
    ] {
        if let Ok(date) = NaiveDate::parse_from_str(s, fmt) {
            return Some(date);
        }
    }

    None
}

/// Parse an amount string, handling currency symbols, commas, and parenthesized negatives.
pub fn parse_amount(s: &str) -> Option<i64> {
    let s = s.trim();

    let (is_negative, s) =
        if let Some(inner) = s.strip_prefix('(').and_then(|s| s.strip_suffix(')')) {
            (true, inner)
        } else if let Some(rest) = s.strip_prefix('-') {
            (true, rest)
        } else {
            (false, s)
        };

    let cleaned: String = s
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == '.' || *c == '-')
        .collect();

    let value: f64 = cleaned.parse().ok()?;
    let cents = (value * 100.0).round() as i64;

    Some(if is_negative { -cents } else { cents })
}

// ── Import commands ──────────────────────────────────────────────────────────

#[derive(Error, Debug)]
pub enum ImportError {
    #[error("{0}")]
    General(String),
    #[error("Account error: {0}")]
    Account(#[from] crate::commands::account_commands::AccountCommandError),
    #[error("Statement '{0}' was already imported (same file contents)")]
    DuplicateStatement(String),
}

// ── Statement-file provenance ────────────────────────────────────────────────

/// Context describing the source statement file for an import, used to link each
/// imported journal entry back to the file it came from.
pub struct StatementFileRef {
    pub file_path: String,
    pub file_name: String,
    pub bank_id: Option<String>,
    pub account_id: Option<String>,
}

impl StatementFileRef {
    /// Build a reference from a file path (file_name derived from the path).
    pub fn from_path(file_path: &str, account_id: Option<String>, bank_id: Option<String>) -> Self {
        let file_name = std::path::Path::new(file_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(file_path)
            .to_string();
        Self {
            file_path: file_path.to_string(),
            file_name,
            bank_id,
            account_id,
        }
    }
}

/// Compute the SHA-256 (hex) of a file's contents, used to dedup re-imported statements.
pub fn hash_file_contents(path: &str) -> std::io::Result<String> {
    let bytes = std::fs::read(path)?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok(hex::encode(hasher.finalize()))
}

/// Record a statement file, keyed by content hash (idempotent).
/// Returns (statement_file_id, already_imported).
pub fn record_statement_file(
    conn: &Connection,
    sf: &StatementFileRef,
    file_hash: &str,
) -> rusqlite::Result<(i64, bool)> {
    if let Ok(id) = conn.query_row(
        "SELECT id FROM statement_files WHERE file_hash = ?1",
        [file_hash],
        |row| row.get::<_, i64>(0),
    ) {
        return Ok((id, true));
    }

    conn.execute(
        "INSERT INTO statement_files (file_path, file_name, file_hash, bank_id, account_id)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![sf.file_path, sf.file_name, file_hash, sf.bank_id, sf.account_id],
    )?;
    Ok((conn.last_insert_rowid(), false))
}

/// Link an imported journal entry to its source statement file.
pub fn link_entry_to_statement(conn: &Connection, entry_id: &str, statement_file_id: i64) {
    let _ = conn.execute(
        "INSERT OR IGNORE INTO entry_statement_files (entry_id, statement_file_id)
         VALUES (?1, ?2)",
        params![entry_id, statement_file_id],
    );
}

/// Extract the entry_id from a posted JournalEntryPosted event.
fn posted_entry_id(stored: &crate::events::types::StoredEvent) -> Option<String> {
    match &stored.event {
        Event::JournalEntryPosted { entry_id, .. } => Some(entry_id.clone()),
        _ => None,
    }
}

/// Parameters for a CSV file import.
pub struct CsvImportParams {
    pub file_path: String,
    pub date_column: usize,
    pub description_column: usize,
    pub amount_column: usize,
    pub target_account_id: String,
    pub target_is_asset: bool,
    pub skip_lines: usize,
    pub has_header: bool,
    pub delimiter: char,
}

/// A parsed transaction ready for import (bank CSV or other source).
pub struct ImportTransaction {
    pub date: NaiveDate,
    pub description: String,
    pub amount: i64, // cents, positive = increase, negative = decrease
}

/// Import transactions from a CSV file.
/// Returns the number of successfully imported transactions.
pub fn import_csv(store: &mut EventStore, params: &CsvImportParams) -> Result<usize, ImportError> {
    let content = std::fs::read_to_string(&params.file_path)
        .map_err(|e| ImportError::General(format!("Failed to read file: {}", e)))?;

    let mut lines = content.lines();

    for _ in 0..params.skip_lines {
        lines.next();
    }
    if params.has_header {
        lines.next();
    }

    let uncategorized_id = find_or_create_uncategorized(store)?;

    // Record the source statement file (dedup by content hash) so each imported
    // entry can be linked back to the file it came from.
    let file_hash = hash_file_contents(&params.file_path)
        .map_err(|e| ImportError::General(format!("Failed to hash file: {}", e)))?;
    let sf = StatementFileRef::from_path(&params.file_path, Some(params.target_account_id.clone()), None);
    let (statement_file_id, already_imported) = record_statement_file(store.connection(), &sf, &file_hash)
        .map_err(|e| ImportError::General(format!("Failed to record statement file: {}", e)))?;
    if already_imported {
        return Err(ImportError::DuplicateStatement(sf.file_name));
    }

    let mut count = 0;
    let mut imported_entry_ids: Vec<String> = Vec::new();
    let mut commands = EntryCommands::new(store, "csv-import".to_string());

    for line in lines {
        let fields = parse_delimited_line(line, params.delimiter);

        let date_str = fields
            .get(params.date_column)
            .map(|s| s.as_str())
            .unwrap_or("");
        let description = fields
            .get(params.description_column)
            .map(|s| s.as_str())
            .unwrap_or("");
        let amount_str = fields
            .get(params.amount_column)
            .map(|s| s.as_str())
            .unwrap_or("");

        let date = match parse_date(date_str) {
            Some(d) => d,
            None => continue,
        };

        let amount = match parse_amount(amount_str) {
            Some(a) if a != 0 => a,
            _ => continue,
        };

        let (target_amount, offset_amount) = if params.target_is_asset {
            (amount, -amount)
        } else {
            (-amount, amount)
        };

        let entry_lines = vec![
            EntryLine {
                account_id: params.target_account_id.clone(),
                amount: target_amount,
                currency: "USD".to_string(),
                exchange_rate: None,
                memo: None,
            },
            EntryLine {
                account_id: uncategorized_id.clone(),
                amount: offset_amount,
                currency: "USD".to_string(),
                exchange_rate: None,
                memo: None,
            },
        ];

        match commands.post_entry(PostEntryCommand {
            date,
            memo: description.to_string(),
            lines: entry_lines,
            reference: Some(sf.file_name.clone()),
            source: Some(JournalEntrySource::Import),
        }) {
            Ok(stored) => {
                if let Some(id) = posted_entry_id(&stored) {
                    imported_entry_ids.push(id);
                }
                count += 1;
            }
            Err(e) => {
                eprintln!("Failed to import row: {}", e);
            }
        }
    }

    // commands holds a &mut borrow on the store for the loop; drop it before
    // touching the connection again to link entries to the statement file.
    drop(commands);
    for entry_id in &imported_entry_ids {
        link_entry_to_statement(store.connection(), entry_id, statement_file_id);
    }

    Ok(count)
}

/// Import bank transactions into the ledger.
/// Returns the number of successfully imported transactions.
pub fn import_bank_transactions(
    store: &mut EventStore,
    target_account_id: &str,
    target_account_type: AccountType,
    transactions: &[ImportTransaction],
    source_file: Option<&StatementFileRef>,
) -> Result<usize, ImportError> {
    let uncategorized_id = find_or_create_uncategorized(store)?;

    let _is_asset = matches!(target_account_type, AccountType::Asset);

    // If a source statement file is provided, record it (dedup by content hash)
    // so each imported entry can be linked back to it.
    let statement_file_id = match source_file {
        Some(sf) => {
            let file_hash = hash_file_contents(&sf.file_path)
                .map_err(|e| ImportError::General(format!("Failed to hash file: {}", e)))?;
            let (id, already_imported) =
                record_statement_file(store.connection(), sf, &file_hash).map_err(|e| {
                    ImportError::General(format!("Failed to record statement file: {}", e))
                })?;
            if already_imported {
                return Err(ImportError::DuplicateStatement(sf.file_name.clone()));
            }
            Some(id)
        }
        None => None,
    };
    let reference = source_file.map(|sf| sf.file_name.clone());

    let mut count = 0;
    let mut imported_entry_ids: Vec<String> = Vec::new();
    let mut commands = EntryCommands::new(store, "bank-import".to_string());

    for txn in transactions {
        let target_amount = txn.amount;
        let offset_amount = -txn.amount;

        let entry_lines = vec![
            EntryLine {
                account_id: target_account_id.to_string(),
                amount: target_amount,
                currency: "USD".to_string(),
                exchange_rate: None,
                memo: None,
            },
            EntryLine {
                account_id: uncategorized_id.clone(),
                amount: offset_amount,
                currency: "USD".to_string(),
                exchange_rate: None,
                memo: None,
            },
        ];

        match commands.post_entry(PostEntryCommand {
            date: txn.date,
            memo: txn.description.clone(),
            lines: entry_lines,
            reference: reference.clone(),
            source: Some(JournalEntrySource::Import),
        }) {
            Ok(stored) => {
                if let Some(id) = posted_entry_id(&stored) {
                    imported_entry_ids.push(id);
                }
                count += 1;
            }
            Err(e) => {
                eprintln!("Failed to import transaction: {}", e);
            }
        }
    }

    drop(commands);
    if let Some(statement_file_id) = statement_file_id {
        for entry_id in &imported_entry_ids {
            link_entry_to_statement(store.connection(), entry_id, statement_file_id);
        }
    }

    Ok(count)
}

/// Mark a bank import as processed and optionally save the bank-account mapping.
pub fn finalize_bank_import(
    store: &EventStore,
    import_id: i64,
    account_id: &str,
    save_mapping: bool,
    imported_count: usize,
) {
    let conn = store.connection();

    if save_mapping {
        let bank_info: Option<(Option<String>, String)> = conn
            .query_row(
                "SELECT bank_id, bank_name FROM pending_imports WHERE id = ?1",
                [import_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .ok();

        if let Some((Some(bank_id), bank_name)) = bank_info {
            let _ = conn.execute(
                "INSERT OR REPLACE INTO bank_accounts (bank_id, bank_name, account_id) VALUES (?1, ?2, ?3)",
                rusqlite::params![bank_id, bank_name, account_id],
            );
        }
    }

    let _ = conn.execute(
        "UPDATE pending_imports SET status = 'imported', imported_count = ?1, processed_at = datetime('now') WHERE id = ?2",
        rusqlite::params![imported_count as i64, import_id],
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::account_commands::{AccountCommands, CreateAccountCommand};
    use crate::store::migrations::init_schema;

    fn setup_with_asset_account() -> (EventStore, String) {
        let mut store = EventStore::in_memory().unwrap();
        init_schema(store.connection()).unwrap();
        let mut commands = AccountCommands::new(&mut store, "user".to_string());
        commands
            .create_account(CreateAccountCommand {
                account_type: AccountType::Asset,
                account_number: "1000".to_string(),
                name: "Checking".to_string(),
                parent_id: None,
                currency: Some("USD".to_string()),
                description: None,
            })
            .unwrap();
        let account_id: String = store
            .connection()
            .query_row(
                "SELECT id FROM accounts WHERE account_number = '1000'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        (store, account_id)
    }

    fn write_temp_csv() -> tempfile::NamedTempFile {
        use std::io::Write;
        let mut f = tempfile::Builder::new().suffix(".csv").tempfile().unwrap();
        writeln!(f, "date,description,amount").unwrap();
        writeln!(f, "2024-01-15,Coffee shop,-4.50").unwrap();
        writeln!(f, "2024-01-16,Paycheck,1000.00").unwrap();
        f.flush().unwrap();
        f
    }

    fn csv_params(file_path: &str, account_id: &str) -> CsvImportParams {
        CsvImportParams {
            file_path: file_path.to_string(),
            date_column: 0,
            description_column: 1,
            amount_column: 2,
            target_account_id: account_id.to_string(),
            target_is_asset: true,
            skip_lines: 0,
            has_header: true,
            delimiter: ',',
        }
    }

    #[test]
    fn test_csv_import_links_statement_file() {
        let (mut store, account_id) = setup_with_asset_account();
        let csv = write_temp_csv();
        let path = csv.path().to_str().unwrap().to_string();

        let count = import_csv(&mut store, &csv_params(&path, &account_id)).unwrap();
        assert_eq!(count, 2);

        // One statement_files row recorded for the imported file.
        let file_count: i64 = store
            .connection()
            .query_row("SELECT COUNT(*) FROM statement_files", [], |r| r.get(0))
            .unwrap();
        assert_eq!(file_count, 1);

        // Every imported entry is linked to that statement file.
        let link_count: i64 = store
            .connection()
            .query_row("SELECT COUNT(*) FROM entry_statement_files", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(link_count, 2);

        // The entry reference carries the file name.
        let file_name = std::path::Path::new(&path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap();
        let ref_count: i64 = store
            .connection()
            .query_row(
                "SELECT COUNT(*) FROM journal_entries WHERE reference = ?1",
                [file_name],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(ref_count, 2);
    }

    #[test]
    fn test_reimporting_same_file_is_rejected() {
        let (mut store, account_id) = setup_with_asset_account();
        let csv = write_temp_csv();
        let path = csv.path().to_str().unwrap().to_string();

        import_csv(&mut store, &csv_params(&path, &account_id)).unwrap();
        let again = import_csv(&mut store, &csv_params(&path, &account_id));
        assert!(matches!(again, Err(ImportError::DuplicateStatement(_))));

        // No duplicate entries were posted.
        let entry_count: i64 = store
            .connection()
            .query_row("SELECT COUNT(*) FROM journal_entries", [], |r| r.get(0))
            .unwrap();
        assert_eq!(entry_count, 2);
    }
}
