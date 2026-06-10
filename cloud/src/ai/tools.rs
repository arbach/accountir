//! Tool definitions exposed to the AI chat: read accounts, list entries,
//! create accounts, post journal entries, get balances.
//!
//! Each tool returns a JSON Value that becomes the `content` of a
//! tool_result block sent back to Claude.

use accountir_core::events::types::EventAccountType;
use chrono::NaiveDate;
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

use crate::commands::{
    account::{create_account, CreateAccountInput},
    entry::{post_entry, EntryLineInput, PostEntryInput},
};
use crate::queries;

pub fn schemas() -> Vec<Value> {
    vec![
        json!({
            "name": "list_accounts",
            "description": "List the company's chart of accounts. Returns id, account_number, name, type, currency for each.",
            "input_schema": {
                "type": "object",
                "properties": {},
            }
        }),
        json!({
            "name": "list_recent_entries",
            "description": "List the most recent journal entries (date, memo, total, reference). Default limit 20, max 100.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "limit": {"type": "integer", "minimum": 1, "maximum": 100, "default": 20}
                }
            }
        }),
        json!({
            "name": "get_account_balance",
            "description": "Return the running debit-positive balance for an account (looked up by account_number, e.g. '1000').",
            "input_schema": {
                "type": "object",
                "properties": {
                    "account_number": {"type": "string"}
                },
                "required": ["account_number"]
            }
        }),
        json!({
            "name": "create_account",
            "description": "Create a new account in the chart. Returns the new account id.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "account_number": {"type": "string", "description": "e.g. '1000'"},
                    "name": {"type": "string", "description": "e.g. 'Cash'"},
                    "type": {"type": "string", "enum": ["asset", "liability", "equity", "revenue", "expense"]},
                    "currency": {"type": "string", "default": "USD"}
                },
                "required": ["account_number", "name", "type"]
            }
        }),
        json!({
            "name": "post_journal_entry",
            "description": "Post a balanced double-entry journal entry. Lines are referred to by account_number. Positive amount = debit, negative = credit. Sum must be zero. Provide amounts in dollars (e.g. 100.00).",
            "input_schema": {
                "type": "object",
                "properties": {
                    "date": {"type": "string", "description": "YYYY-MM-DD"},
                    "memo": {"type": "string"},
                    "reference": {"type": "string"},
                    "lines": {
                        "type": "array",
                        "minItems": 2,
                        "items": {
                            "type": "object",
                            "properties": {
                                "account_number": {"type": "string"},
                                "amount": {"type": "number", "description": "Dollars; positive=debit, negative=credit"},
                                "currency": {"type": "string", "default": "USD"},
                                "memo": {"type": "string"}
                            },
                            "required": ["account_number", "amount"]
                        }
                    }
                },
                "required": ["date", "memo", "lines"]
            }
        }),
        json!({
            "name": "trial_balance",
            "description": "Trial balance: every account with total debits and credits (in dollars), plus grand totals.",
            "input_schema": { "type": "object", "properties": {} }
        }),
        json!({
            "name": "income_statement",
            "description": "Income statement (P&L) for a date range: revenue and expense lines plus net income.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "start": {"type": "string", "description": "YYYY-MM-DD"},
                    "end": {"type": "string", "description": "YYYY-MM-DD"}
                },
                "required": ["start", "end"]
            }
        }),
        json!({
            "name": "balance_sheet",
            "description": "Balance sheet as of a date: assets, liabilities, equity, net income, and whether it balances.",
            "input_schema": {
                "type": "object",
                "properties": { "as_of": {"type": "string", "description": "YYYY-MM-DD"} },
                "required": ["as_of"]
            }
        }),
        json!({
            "name": "cash_flow",
            "description": "Simplified cash flow for a date range: opening/closing cash and change broken down by counterpart account.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "start": {"type": "string", "description": "YYYY-MM-DD"},
                    "end": {"type": "string", "description": "YYYY-MM-DD"}
                },
                "required": ["start", "end"]
            }
        }),
        json!({
            "name": "list_transactions",
            "description": "List transaction lines, optionally filtered by date range, account_number, memo search, direction, or amount range. Amounts in dollars (positive=debit). Default limit 50, max 200.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "start": {"type": "string", "description": "YYYY-MM-DD"},
                    "end": {"type": "string", "description": "YYYY-MM-DD"},
                    "account_number": {"type": "string"},
                    "search": {"type": "string", "description": "substring match on memo"},
                    "direction": {"type": "string", "enum": ["debit", "credit"], "description": "debit = money in on the bank side, credit = money out"},
                    "min_amount": {"type": "number", "description": "minimum absolute amount in dollars"},
                    "max_amount": {"type": "number", "description": "maximum absolute amount in dollars"},
                    "limit": {"type": "integer", "minimum": 1, "maximum": 200, "default": 50}
                }
            }
        }),
        json!({
            "name": "list_bank_connections",
            "description": "List connected bank (Plaid) items: id, institution, status, last sync time.",
            "input_schema": { "type": "object", "properties": {} }
        }),
        json!({
            "name": "get_tax_profile",
            "description": "Get the company's tax profile (entity type, legal name, EIN, mailing address). Needed before filling or mailing tax forms.",
            "input_schema": { "type": "object", "properties": {} }
        }),
        json!({
            "name": "set_tax_profile",
            "description": "Save the company's tax profile. Ask the user for any values you don't have — never invent an EIN or address.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "entity_type": {"type": "string", "enum": ["schedule_c", "s_corp", "partnership", "c_corp"]},
                    "legal_name": {"type": "string"},
                    "ein": {"type": "string"},
                    "address": {"type": "object", "properties": {
                        "line1": {"type": "string"}, "line2": {"type": "string"},
                        "city": {"type": "string"}, "state": {"type": "string"}, "zip": {"type": "string"}
                    }}
                },
                "required": ["entity_type", "legal_name", "address"]
            }
        }),
        json!({
            "name": "fetch_tax_form",
            "description": "Download an official IRS form PDF from irs.gov and register it in the Tax Filing pipeline. form is the IRS file code, e.g. f1040sc (Schedule C), f1120s (1120-S), f1065, f1099nec, f4562. Returns the form_id and its fillable field names.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "form": {"type": "string", "description": "IRS form file code, lowercase, e.g. f1040sc"},
                    "year": {"type": "integer", "description": "Tax year this filing is for"},
                    "title": {"type": "string", "description": "Optional display title"}
                },
                "required": ["form", "year"]
            }
        }),
        json!({
            "name": "get_tax_form_fields",
            "description": "List the fillable fields (names, types, current values, checkbox states) of a pulled tax form.",
            "input_schema": {
                "type": "object",
                "properties": { "form_id": {"type": "string"} },
                "required": ["form_id"]
            }
        }),
        json!({
            "name": "fill_tax_form",
            "description": "Fill values into a pulled tax form's PDF fields (cumulative — only the fields you pass change). values maps exact field names from get_tax_form_fields to strings; for checkboxes pass true/false. After filling, tell the user to review and Approve the form on /app/tax before it can be mailed.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "form_id": {"type": "string"},
                    "values": {"type": "object", "description": "field name -> value"}
                },
                "required": ["form_id", "values"]
            }
        }),
        json!({
            "name": "list_tax_forms",
            "description": "List the company's tax forms in the filing pipeline with their status (pulled, filled, approved, mailed) and Lob mailing info.",
            "input_schema": { "type": "object", "properties": {} }
        }),
        json!({
            "name": "mail_tax_form",
            "description": "Physically mail an APPROVED tax form via Lob (print + post). Refuses forms the user hasn't approved on /app/tax. Provide the destination address (e.g. the correct IRS service center for the form and the company's state — verify it on irs.gov first). Use certified=true for tax returns. Always restate the destination and get an explicit yes from the user in chat before calling this.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "form_id": {"type": "string"},
                    "to": {"type": "object", "properties": {
                        "name": {"type": "string"}, "address_line1": {"type": "string"},
                        "address_line2": {"type": "string"}, "address_city": {"type": "string"},
                        "address_state": {"type": "string"}, "address_zip": {"type": "string"}
                    }, "required": ["name", "address_line1", "address_city", "address_state", "address_zip"]},
                    "certified": {"type": "boolean", "default": true}
                },
                "required": ["form_id", "to"]
            }
        }),
        json!({
            "name": "create_report",
            "description": "Generate a saved report document that appears under Reports → Tax Documents and can be saved as PDF. Use type 'tax_package' to complete the full year-end tax documents (income statement + balance sheet + cash flow + trial balance for a year). Returns the document URL — navigate the user there afterwards.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "type": {"type": "string", "enum": ["income_statement", "balance_sheet", "cash_flow", "trial_balance", "tax_package"]},
                    "year": {"type": "integer", "description": "Fiscal year (defaults to current). Used for tax_package and as the default range."},
                    "start": {"type": "string", "description": "YYYY-MM-DD (optional, overrides year)"},
                    "end": {"type": "string", "description": "YYYY-MM-DD (optional, overrides year)"},
                    "as_of": {"type": "string", "description": "YYYY-MM-DD for balance_sheet (optional)"},
                    "title": {"type": "string", "description": "Optional custom title"}
                },
                "required": ["type"]
            }
        }),
        json!({
            "name": "void_entry",
            "description": "Void a journal entry (reversible). Use this to undo a posted entry instead of posting a manual reversal. Get the entry_id from list_recent_entries or list_transactions.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "entry_id": {"type": "string", "description": "UUID of the journal entry"},
                    "reason": {"type": "string", "description": "Why it's being voided"}
                },
                "required": ["entry_id"]
            }
        }),
        json!({
            "name": "unvoid_entry",
            "description": "Restore a previously voided journal entry.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "entry_id": {"type": "string", "description": "UUID of the journal entry"},
                    "reason": {"type": "string"}
                },
                "required": ["entry_id"]
            }
        }),
        json!({
            "name": "navigate_to_page",
            "description": "Navigate the user's browser to a page of this app. Use when the user asks to see/open/go to a page, or to show them the result of your work. Available pages: /app/dashboard, /app/invoices (sales), /app/transactions, /app/entries (journal), /app/accounts (chart of accounts), /app/banks, /app/banks/link, /app/reports, /app/reports/trial-balance, /app/reports/income-statement?start=YYYY-MM-DD&end=YYYY-MM-DD, /app/reports/balance-sheet?as_of=YYYY-MM-DD, /app/reports/cash-flow?start=YYYY-MM-DD&end=YYYY-MM-DD, /app/reports/tax-documents, /app/reports/documents/<id>, /app/tax, /app/chat, /app/admin/companies, /app/admin/members, /app/admin/settings.",
            "input_schema": {
                "type": "object",
                "properties": { "page": {"type": "string", "description": "App path starting with /app/, optionally with query params"} },
                "required": ["page"]
            }
        }),
        json!({
            "name": "sync_bank",
            "description": "Sync transactions from a connected bank item (by item id from list_bank_connections). Imports new bank transactions into the ledger.",
            "input_schema": {
                "type": "object",
                "properties": { "item_id": {"type": "string", "description": "UUID of the bank connection"} },
                "required": ["item_id"]
            }
        }),
    ]
}

pub struct ToolContext<'a> {
    pub pool: &'a PgPool,
    pub company_id: Uuid,
    pub user_id: Uuid,
}

pub async fn execute(name: &str, input: &Value, ctx: &ToolContext<'_>) -> Value {
    match name {
        "list_accounts" => list_accounts_tool(ctx).await,
        "list_recent_entries" => list_recent_entries_tool(ctx, input).await,
        "get_account_balance" => get_account_balance_tool(ctx, input).await,
        "create_account" => create_account_tool(ctx, input).await,
        "post_journal_entry" => post_entry_tool(ctx, input).await,
        "create_report" => create_report_tool(ctx, input).await,
        "get_tax_profile" => get_tax_profile_tool(ctx).await,
        "set_tax_profile" => set_tax_profile_tool(ctx, input).await,
        "fetch_tax_form" => fetch_tax_form_tool(ctx, input).await,
        "get_tax_form_fields" => tax_form_fields_tool(ctx, input).await,
        "fill_tax_form" => fill_tax_form_tool(ctx, input).await,
        "list_tax_forms" => list_tax_forms_tool(ctx).await,
        "mail_tax_form" => mail_tax_form_tool(ctx, input).await,
        "void_entry" => void_entry_tool(ctx, input, false).await,
        "unvoid_entry" => void_entry_tool(ctx, input, true).await,
        "trial_balance" => trial_balance_tool(ctx).await,
        "income_statement" => income_statement_tool(ctx, input).await,
        "balance_sheet" => balance_sheet_tool(ctx, input).await,
        "cash_flow" => cash_flow_tool(ctx, input).await,
        "list_transactions" => list_transactions_tool(ctx, input).await,
        "list_bank_connections" => list_bank_connections_tool(ctx).await,
        "navigate_to_page" => navigate_tool(input),
        // NOTE: "sync_bank" needs AppState (plaid config) and is handled by the
        // MCP route layer before delegating here.
        other => json!({ "error": format!("unknown tool: {other}") }),
    }
}

fn dollars(cents: i64) -> f64 {
    cents as f64 / 100.0
}

fn parse_date(input: &Value, key: &str) -> Result<NaiveDate, Value> {
    input
        .get(key)
        .and_then(|v| v.as_str())
        .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
        .ok_or_else(|| json!({ "error": format!("{key} must be YYYY-MM-DD") }))
}

fn report_lines(lines: &[queries::ReportLine]) -> Vec<Value> {
    lines
        .iter()
        .map(|l| {
            json!({
                "account_number": l.account_number,
                "name": l.name,
                "amount": dollars(l.amount_cents),
            })
        })
        .collect()
}

async fn trial_balance_tool(ctx: &ToolContext<'_>) -> Value {
    match queries::trial_balance(ctx.pool, ctx.company_id).await {
        Ok((rows, total_debit, total_credit)) => json!({
            "rows": rows.iter().map(|r| json!({
                "account_number": r.account_number,
                "name": r.name,
                "type": r.account_type,
                "debit": dollars(r.debit_cents),
                "credit": dollars(r.credit_cents),
            })).collect::<Vec<_>>(),
            "total_debit": dollars(total_debit),
            "total_credit": dollars(total_credit),
            "balanced": total_debit == total_credit,
        }),
        Err(e) => json!({ "error": format!("{e}") }),
    }
}

async fn income_statement_tool(ctx: &ToolContext<'_>, input: &Value) -> Value {
    let start = match parse_date(input, "start") { Ok(d) => d, Err(e) => return e };
    let end = match parse_date(input, "end") { Ok(d) => d, Err(e) => return e };
    match queries::income_statement(ctx.pool, ctx.company_id, start, end).await {
        Ok(r) => json!({
            "start": r.start.to_string(),
            "end": r.end.to_string(),
            "revenues": report_lines(&r.revenues),
            "expenses": report_lines(&r.expenses),
            "total_revenue": dollars(r.total_revenue_cents),
            "total_expense": dollars(r.total_expense_cents),
            "net_income": dollars(r.net_income_cents()),
        }),
        Err(e) => json!({ "error": format!("{e}") }),
    }
}

async fn balance_sheet_tool(ctx: &ToolContext<'_>, input: &Value) -> Value {
    let as_of = match parse_date(input, "as_of") { Ok(d) => d, Err(e) => return e };
    match queries::balance_sheet(ctx.pool, ctx.company_id, as_of).await {
        Ok(r) => json!({
            "as_of": r.as_of.to_string(),
            "assets": report_lines(&r.assets),
            "liabilities": report_lines(&r.liabilities),
            "equity": report_lines(&r.equity),
            "net_income": dollars(r.net_income_cents),
            "total_assets": dollars(r.total_assets_cents),
            "total_liabilities": dollars(r.total_liab_cents),
            "total_equity": dollars(r.total_equity_cents),
            "balanced": r.total_assets_cents == r.liab_plus_equity_cents(),
        }),
        Err(e) => json!({ "error": format!("{e}") }),
    }
}

async fn cash_flow_tool(ctx: &ToolContext<'_>, input: &Value) -> Value {
    let start = match parse_date(input, "start") { Ok(d) => d, Err(e) => return e };
    let end = match parse_date(input, "end") { Ok(d) => d, Err(e) => return e };
    match queries::cash_flow(ctx.pool, ctx.company_id, start, end).await {
        Ok(r) => json!({
            "start": r.start.to_string(),
            "end": r.end.to_string(),
            "opening_cash": dollars(r.opening_cash_cents),
            "closing_cash": dollars(r.closing_cash_cents),
            "change": dollars(r.change_cents),
            "by_counterpart_account": report_lines(&r.by_other_account),
        }),
        Err(e) => json!({ "error": format!("{e}") }),
    }
}

async fn list_transactions_tool(ctx: &ToolContext<'_>, input: &Value) -> Value {
    let account_id = match input.get("account_number").and_then(|v| v.as_str()) {
        Some(num) => {
            // Resolve via the tenant-aware query path (accounts table is FORCE RLS).
            let accounts = match queries::list_accounts(ctx.pool, ctx.company_id).await {
                Ok(a) => a,
                Err(e) => return json!({ "error": format!("{e}") }),
            };
            match accounts.iter().find(|a| a.account_number == num) {
                Some(a) => Some(a.id),
                None => return json!({ "error": format!("no account with number '{num}'") }),
            }
        }
        None => None,
    };
    let filter = queries::TransactionFilter {
        start: input.get("start").and_then(|v| v.as_str()).and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()),
        end: input.get("end").and_then(|v| v.as_str()).and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()),
        account_id,
        source: None,
        search: input.get("search").and_then(|v| v.as_str()).map(str::to_string),
        include_void: false,
        direction: input
            .get("direction")
            .and_then(|v| v.as_str())
            .filter(|s| *s == "debit" || *s == "credit")
            .map(str::to_string),
        min_cents: input.get("min_amount").and_then(|v| v.as_f64()).map(|v| (v * 100.0).round() as i64),
        max_cents: input.get("max_amount").and_then(|v| v.as_f64()).map(|v| (v * 100.0).round() as i64),
    };
    let limit = input.get("limit").and_then(|v| v.as_u64()).unwrap_or(50).min(200) as usize;
    match queries::list_transactions(ctx.pool, ctx.company_id, &filter).await {
        Ok(lines) => json!({
            "count": lines.len().min(limit),
            "total_matching": lines.len(),
            "transactions": lines.iter().take(limit).map(|t| json!({
                "line_id": t.line_id,
                "entry_id": t.entry_id,
                "date": t.date.to_string(),
                "memo": t.memo,
                "account_number": t.account_number,
                "account": t.account_name,
                "amount": dollars(t.amount_cents),
                "is_void": t.is_void,
            })).collect::<Vec<_>>(),
        }),
        Err(e) => json!({ "error": format!("{e}") }),
    }
}

/// The browser performs the actual navigation when it sees this tool_use in
/// the live event stream; server-side we only validate the target so the
/// agent can never send the user off-app.
fn navigate_tool(input: &Value) -> Value {
    let Some(page) = input.get("page").and_then(|v| v.as_str()) else {
        return json!({ "error": "page required" });
    };
    let (path, query) = page.split_once('?').unwrap_or((page, ""));
    let path_ok = path.starts_with("/app/")
        && !path.contains("..")
        && !path.contains("//")
        && path
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '-' | '_'));
    let query_ok = query
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '=' | '&' | '-' | '_' | '%' | '.'));
    if !path_ok || !query_ok {
        return json!({ "error": "invalid page: must be an in-app path starting with /app/" });
    }
    json!({ "ok": true, "navigating_to": page, "note": "the user's browser is switching to this page now" })
}

async fn list_bank_connections_tool(ctx: &ToolContext<'_>) -> Value {
    match queries::list_plaid_items(ctx.pool, ctx.company_id).await {
        Ok(items) => json!({
            "items": items.iter().map(|i| json!({
                "item_id": i.id,
                "institution": i.institution_name,
                "status": i.status,
                "last_synced": i.last_synced_display(),
            })).collect::<Vec<_>>(),
        }),
        Err(e) => json!({ "error": format!("{e}") }),
    }
}

async fn list_accounts_tool(ctx: &ToolContext<'_>) -> Value {
    match queries::list_accounts(ctx.pool, ctx.company_id).await {
        Ok(rows) => json!({
            "accounts": rows.iter().map(|a| json!({
                "id": a.id,
                "account_number": a.account_number,
                "name": a.name,
                "type": a.account_type,
                "currency": a.currency,
            })).collect::<Vec<_>>()
        }),
        Err(e) => json!({ "error": format!("{e}") }),
    }
}

async fn list_recent_entries_tool(ctx: &ToolContext<'_>, input: &Value) -> Value {
    let limit = input
        .get("limit")
        .and_then(|v| v.as_i64())
        .unwrap_or(20)
        .clamp(1, 100);
    match queries::list_entries(ctx.pool, ctx.company_id).await {
        Ok(rows) => {
            let truncated: Vec<_> = rows
                .into_iter()
                .take(limit as usize)
                .map(|e| {
                    json!({
                        "id": e.id,
                        "date": e.date,
                        "memo": e.memo,
                        "reference": e.reference,
                        "total_debits": format!("{:.2}", e.total_debits_cents as f64 / 100.0),
                    })
                })
                .collect();
            json!({ "entries": truncated })
        }
        Err(e) => json!({ "error": format!("{e}") }),
    }
}

async fn get_account_balance_tool(ctx: &ToolContext<'_>, input: &Value) -> Value {
    let acct_num = match input.get("account_number").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => return json!({ "error": "missing account_number" }),
    };
    // accounts/journal_lines are FORCE RLS — queries must run in a tenant-scoped tx.
    let row: Option<(Uuid, String, i64)> = async {
        let mut conn = ctx.pool.acquire().await.ok()?;
        let mut tx = sqlx::Acquire::begin(&mut *conn).await.ok()?;
        crate::store::event_store::set_tenant(&mut tx, ctx.company_id).await.ok()?;
        let row: Option<(Uuid, String, i64)> = sqlx::query_as(
            r#"
            SELECT a.id, a.name,
                   COALESCE(SUM(jl.amount), 0)::BIGINT
            FROM accounts a
            LEFT JOIN journal_lines jl ON jl.account_id = a.id
            LEFT JOIN journal_entries je ON je.id = jl.entry_id AND je.is_void = false
            WHERE a.company_id = $1 AND a.account_number = $2 AND a.is_active = true
            GROUP BY a.id, a.name
            "#,
        )
        .bind(ctx.company_id)
        .bind(acct_num)
        .fetch_optional(&mut *tx)
        .await
        .ok()
        .flatten();
        tx.commit().await.ok();
        row
    }
    .await;
    match row {
        Some((id, name, cents)) => json!({
            "account_id": id,
            "account_number": acct_num,
            "name": name,
            "balance": format!("{:.2}", cents as f64 / 100.0),
            "balance_cents": cents,
        }),
        None => json!({ "error": format!("no active account with number '{acct_num}'") }),
    }
}

async fn create_account_tool(ctx: &ToolContext<'_>, input: &Value) -> Value {
    let acct_num = input
        .get("account_number")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let name = input.get("name").and_then(|v| v.as_str()).map(str::to_string);
    let typ_str = input.get("type").and_then(|v| v.as_str()).unwrap_or("");
    let currency = input
        .get("currency")
        .and_then(|v| v.as_str())
        .map(str::to_uppercase);
    let (acct_num, name) = match (acct_num, name) {
        (Some(a), Some(n)) => (a, n),
        _ => return json!({ "error": "account_number and name required" }),
    };
    let typ = match typ_str {
        "asset" => EventAccountType::Asset,
        "liability" => EventAccountType::Liability,
        "equity" => EventAccountType::Equity,
        "revenue" => EventAccountType::Revenue,
        "expense" => EventAccountType::Expense,
        _ => return json!({ "error": "type must be one of: asset, liability, equity, revenue, expense" }),
    };
    match create_account(
        ctx.pool,
        ctx.company_id,
        ctx.user_id,
        CreateAccountInput {
            account_type: typ,
            account_number: acct_num.clone(),
            name: name.clone(),
            currency,
            description: None,
        },
    )
    .await
    {
        Ok(id) => json!({ "ok": true, "account_id": id, "account_number": acct_num, "name": name }),
        Err(e) => json!({ "error": format!("{e}") }),
    }
}

async fn get_tax_profile_tool(ctx: &ToolContext<'_>) -> Value {
    match crate::tax::get_profile(ctx.pool, ctx.company_id).await {
        Ok(Some(p)) => json!({
            "entity_type": p.entity_type, "legal_name": p.legal_name,
            "ein": p.ein, "address": p.address,
        }),
        Ok(None) => json!({ "profile": null, "note": "no tax profile yet — ask the user and call set_tax_profile" }),
        Err(e) => json!({ "error": format!("{e}") }),
    }
}

async fn set_tax_profile_tool(ctx: &ToolContext<'_>, input: &Value) -> Value {
    let entity_type = input.get("entity_type").and_then(|v| v.as_str()).unwrap_or("");
    let legal_name = input.get("legal_name").and_then(|v| v.as_str()).unwrap_or("");
    let ein = input.get("ein").and_then(|v| v.as_str()).unwrap_or("");
    let address = input.get("address").cloned().unwrap_or_else(|| json!({}));
    if entity_type.is_empty() || legal_name.is_empty() {
        return json!({ "error": "entity_type and legal_name are required" });
    }
    match crate::tax::set_profile(ctx.pool, ctx.company_id, entity_type, legal_name, ein, &address)
        .await
    {
        Ok(()) => json!({ "ok": true }),
        Err(e) => json!({ "error": format!("{e}") }),
    }
}

async fn fetch_tax_form_tool(ctx: &ToolContext<'_>, input: &Value) -> Value {
    let form = input.get("form").and_then(|v| v.as_str()).unwrap_or("");
    let year = input.get("year").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
    let title = input.get("title").and_then(|v| v.as_str());
    if form.is_empty() || year < 2000 {
        return json!({ "error": "form and year are required" });
    }
    match crate::tax::fetch_form(ctx.pool, ctx.company_id, form, year, title).await {
        Ok((id, fields)) => json!({
            "ok": true, "form_id": id, "fields": fields,
            "review_url": format!("/app/tax/forms/{id}/pdf"),
        }),
        Err(e) => json!({ "error": format!("{e}") }),
    }
}

async fn tax_form_fields_tool(ctx: &ToolContext<'_>, input: &Value) -> Value {
    let Some(id) = input.get("form_id").and_then(|v| v.as_str()).and_then(|s| Uuid::parse_str(s).ok())
    else {
        return json!({ "error": "form_id must be a UUID" });
    };
    match crate::tax::form_fields(ctx.pool, ctx.company_id, id).await {
        Ok(fields) => fields,
        Err(e) => json!({ "error": format!("{e}") }),
    }
}

async fn fill_tax_form_tool(ctx: &ToolContext<'_>, input: &Value) -> Value {
    let Some(id) = input.get("form_id").and_then(|v| v.as_str()).and_then(|s| Uuid::parse_str(s).ok())
    else {
        return json!({ "error": "form_id must be a UUID" });
    };
    let Some(values) = input.get("values").filter(|v| v.is_object()) else {
        return json!({ "error": "values object required" });
    };
    match crate::tax::fill_form(ctx.pool, ctx.company_id, id, values).await {
        Ok(res) => json!({
            "ok": true, "result": res,
            "review_url": format!("/app/tax/forms/{id}/pdf"),
            "next": "ask the user to review the PDF and click Approve on /app/tax",
        }),
        Err(e) => json!({ "error": format!("{e}") }),
    }
}

async fn list_tax_forms_tool(ctx: &ToolContext<'_>) -> Value {
    match crate::tax::list_forms(ctx.pool, ctx.company_id).await {
        Ok(rows) => json!({
            "lob_configured": crate::tax::lob::configured(),
            "forms": rows.iter().map(|f| json!({
                "form_id": f.id, "year": f.year, "form": f.form_code, "title": f.title,
                "status": f.status, "lob_id": f.lob_id, "lob_status": f.lob_status,
                "mailed_at": f.mailed_at.map(|t| t.to_rfc3339()),
                "review_url": format!("/app/tax/forms/{}/pdf", f.id),
            })).collect::<Vec<_>>(),
        }),
        Err(e) => json!({ "error": format!("{e}") }),
    }
}

async fn mail_tax_form_tool(ctx: &ToolContext<'_>, input: &Value) -> Value {
    let Some(id) = input.get("form_id").and_then(|v| v.as_str()).and_then(|s| Uuid::parse_str(s).ok())
    else {
        return json!({ "error": "form_id must be a UUID" });
    };
    let Some(to) = input.get("to").filter(|v| v.is_object()) else {
        return json!({ "error": "to address object required" });
    };
    let certified = input.get("certified").and_then(|v| v.as_bool()).unwrap_or(true);
    match crate::tax::mail_form(ctx.pool, ctx.company_id, id, to, certified).await {
        Ok(letter) => json!({
            "ok": true,
            "lob_id": letter.get("id"),
            "expected_delivery_date": letter.get("expected_delivery_date"),
            "tracking_number": letter.get("tracking_number"),
            "carrier": letter.get("carrier"),
        }),
        Err(e) => json!({ "error": format!("{e}") }),
    }
}

async fn create_report_tool(ctx: &ToolContext<'_>, input: &Value) -> Value {
    let doc_type = input.get("type").and_then(|v| v.as_str()).unwrap_or("");
    let date_of = |k: &str| {
        input
            .get(k)
            .and_then(|v| v.as_str())
            .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
    };
    let year = input.get("year").and_then(|v| v.as_i64()).map(|y| y as i32);
    match crate::docgen::generate(
        ctx.pool,
        ctx.company_id,
        doc_type,
        date_of("start"),
        date_of("end"),
        date_of("as_of"),
        year,
    )
    .await
    {
        Ok((default_title, kind, html)) => {
            let title = input
                .get("title")
                .and_then(|v| v.as_str())
                .filter(|s| !s.trim().is_empty())
                .unwrap_or(&default_title);
            match crate::docgen::save_document(ctx.pool, ctx.company_id, &kind, title, &html).await
            {
                Ok(id) => json!({
                    "ok": true,
                    "document_id": id,
                    "title": title,
                    "url": format!("/app/reports/documents/{id}"),
                    "tab": "/app/reports/tax-documents"
                }),
                Err(e) => json!({ "error": format!("save failed: {e}") }),
            }
        }
        Err(e) => json!({ "error": format!("{e}") }),
    }
}

async fn void_entry_tool(ctx: &ToolContext<'_>, input: &Value, unvoid: bool) -> Value {
    let Some(entry_id) = input
        .get("entry_id")
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
    else {
        return json!({ "error": "entry_id must be a UUID" });
    };
    let reason = input
        .get("reason")
        .and_then(|v| v.as_str())
        .unwrap_or(if unvoid { "unvoided by AI agent" } else { "voided by AI agent" })
        .to_string();
    let res = if unvoid {
        crate::commands::mutations::unvoid_entry(ctx.pool, ctx.company_id, ctx.user_id, entry_id, reason).await
    } else {
        crate::commands::mutations::void_entry(ctx.pool, ctx.company_id, ctx.user_id, entry_id, reason).await
    };
    match res {
        Ok(_) => json!({ "ok": true, "entry_id": entry_id, "voided": !unvoid }),
        Err(e) => json!({ "error": format!("{e}") }),
    }
}

async fn post_entry_tool(ctx: &ToolContext<'_>, input: &Value) -> Value {
    let date_str = input.get("date").and_then(|v| v.as_str()).unwrap_or("");
    let memo = input
        .get("memo")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let reference = input
        .get("reference")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let date = match NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
        Ok(d) => d,
        Err(_) => return json!({ "error": "date must be YYYY-MM-DD" }),
    };
    let lines_value = input.get("lines").and_then(|v| v.as_array());
    let lines_value = match lines_value {
        Some(l) => l,
        None => return json!({ "error": "lines required" }),
    };
    if lines_value.len() < 2 {
        return json!({ "error": "need at least 2 lines" });
    }

    // Resolve account_numbers → uuids and convert dollar amounts to cents.
    // accounts has FORCE RLS: lookups must run inside a tenant-scoped tx or
    // they silently match nothing.
    let mut conn = match ctx.pool.acquire().await {
        Ok(c) => c,
        Err(e) => return json!({ "error": format!("db: {e}") }),
    };
    let mut tx = match sqlx::Acquire::begin(&mut conn).await {
        Ok(t) => t,
        Err(e) => return json!({ "error": format!("db: {e}") }),
    };
    if let Err(e) = crate::store::event_store::set_tenant(&mut tx, ctx.company_id).await {
        return json!({ "error": format!("db: {e}") });
    }
    let mut input_lines: Vec<EntryLineInput> = Vec::with_capacity(lines_value.len());
    let mut sum_cents = 0i64;
    for line in lines_value {
        let acct_num = match line.get("account_number").and_then(|v| v.as_str()) {
            Some(s) => s,
            None => return json!({ "error": "each line needs account_number" }),
        };
        let amount = match line.get("amount").and_then(|v| v.as_f64()) {
            Some(a) => a,
            None => return json!({ "error": "each line needs numeric amount" }),
        };
        let amount_cents = (amount * 100.0).round() as i64;
        sum_cents += amount_cents;
        let currency = line
            .get("currency")
            .and_then(|v| v.as_str())
            .unwrap_or("USD")
            .to_uppercase();
        let memo_l = line.get("memo").and_then(|v| v.as_str()).map(str::to_string);
        let acct_uuid: Option<(Uuid,)> = sqlx::query_as(
            "SELECT id FROM accounts WHERE company_id = $1 AND account_number = $2 AND is_active = true",
        )
        .bind(ctx.company_id)
        .bind(acct_num)
        .fetch_optional(&mut *tx)
        .await
        .ok()
        .flatten();
        let account_id = match acct_uuid {
            Some((id,)) => id,
            None => return json!({ "error": format!("no active account with number '{acct_num}'") }),
        };
        input_lines.push(EntryLineInput {
            account_id,
            amount: amount_cents,
            currency,
            memo: memo_l,
        });
    }
    drop(tx);
    if sum_cents != 0 {
        return json!({
            "error": format!("lines must balance to zero (got {} cents off)", sum_cents),
        });
    }
    match post_entry(
        ctx.pool,
        ctx.company_id,
        ctx.user_id,
        PostEntryInput {
            date,
            memo,
            reference,
            lines: input_lines,
        },
    )
    .await
    {
        Ok(id) => json!({ "ok": true, "entry_id": id }),
        Err(e) => json!({ "error": format!("{e}") }),
    }
}
