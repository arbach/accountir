//! One-off maintenance: replace bundled journal entries with per-source-document
//! entries, atomically, through the proper event-sourced path (void events +
//! AccountCreated events + JournalEntryPosted events, all in ONE transaction —
//! the whole plan lands or none of it does).
//!
//! Usage: MAINT_PLAN=/path/plan.json cargo run --release --bin maint_unbundle
//!
//! Plan format:
//! {
//!   "company": "<uuid>", "user": "<uuid>",
//!   "void": [{"entry": "<uuid>", "reason": "..."}],
//!   "accounts": [{"number": "1110", "name": "...", "type": "asset", "description": "..."}],
//!   "entries": [{"date": "YYYY-MM-DD", "memo": "...", "reference": "...",
//!                "lines": [{"account": "<account_number>", "amount_cents": -123, "memo": "..."}]}]
//! }
//! Accounts are looked up by account_number in the company; listed accounts are
//! created only if missing. Line "account" refers to an account_number.
use accountir_cloud::commands::account::{create_account_in_tx, CreateAccountInput};
use accountir_cloud::commands::entry::{post_entry_in_tx, EntryLineInput, PostEntryInput};
use accountir_cloud::commands::mutations::void_entry_in_tx;
use accountir_cloud::store::event_store::{append_event, set_tenant};
use accountir_core::events::types::{Event, EventAccountType, JournalEntrySource};
use serde::Deserialize;
use sqlx::postgres::PgPoolOptions;
use sqlx::Acquire;
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Deserialize)]
struct Plan {
    company: Uuid,
    user: Uuid,
    #[serde(default)]
    void: Vec<VoidSpec>,
    #[serde(default)]
    accounts: Vec<AccountSpec>,
    #[serde(default)]
    entries: Vec<EntrySpec>,
    /// Repoint an existing journal line at a different account (event-sourced
    /// JournalLineReassigned) — for moving generic-offset legs to specific accounts.
    #[serde(default)]
    reassigns: Vec<ReassignSpec>,
}
#[derive(Deserialize)]
struct ReassignSpec {
    line: Uuid,
    to_account: String,
}
#[derive(Deserialize)]
struct VoidSpec {
    entry: Uuid,
    reason: String,
}
#[derive(Deserialize)]
struct AccountSpec {
    number: String,
    name: String,
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    description: Option<String>,
}
#[derive(Deserialize)]
struct EntrySpec {
    date: chrono::NaiveDate,
    memo: String,
    #[serde(default)]
    reference: Option<String>,
    lines: Vec<LineSpec>,
}
#[derive(Deserialize)]
struct LineSpec {
    account: String,
    amount_cents: i64,
    #[serde(default)]
    memo: Option<String>,
}

fn account_type(s: &str) -> anyhow::Result<EventAccountType> {
    Ok(match s {
        "asset" => EventAccountType::Asset,
        "liability" => EventAccountType::Liability,
        "equity" => EventAccountType::Equity,
        "revenue" => EventAccountType::Revenue,
        "expense" => EventAccountType::Expense,
        other => anyhow::bail!("unknown account type '{other}'"),
    })
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let url = std::env::var("DATABASE_URL")?;
    let plan_path = std::env::var("MAINT_PLAN")?;
    let plan: Plan = serde_json::from_str(&std::fs::read_to_string(&plan_path)?)?;

    // Every entry in the plan must balance before we touch the DB.
    for (i, e) in plan.entries.iter().enumerate() {
        let sum: i64 = e.lines.iter().map(|l| l.amount_cents).sum();
        anyhow::ensure!(sum == 0, "plan entry {i} ('{}') does not balance: {sum}", e.memo);
    }

    let pool = PgPoolOptions::new().max_connections(2).connect(&url).await?;
    let mut conn = pool.acquire().await?;
    let mut tx = conn.begin().await?;
    set_tenant(&mut tx, plan.company).await?;

    // Resolve account_number -> id for this company; create the listed ones if absent.
    let rows: Vec<(String, Uuid)> =
        sqlx::query_as("SELECT account_number, id FROM accounts WHERE is_active = true")
            .fetch_all(&mut *tx)
            .await?;
    let mut by_number: HashMap<String, Uuid> = rows.into_iter().collect();
    for a in &plan.accounts {
        if by_number.contains_key(&a.number) {
            continue;
        }
        let id = create_account_in_tx(
            &mut tx,
            plan.company,
            plan.user,
            CreateAccountInput {
                account_type: account_type(&a.kind)?,
                account_number: a.number.clone(),
                name: a.name.clone(),
                currency: Some("USD".into()),
                description: a.description.clone(),
            },
        )
        .await?;
        println!("created account {} {} ({id})", a.number, a.name);
        by_number.insert(a.number.clone(), id);
    }

    for v in &plan.void {
        void_entry_in_tx(&mut tx, plan.company, plan.user, v.entry, v.reason.clone()).await?;
        println!("voided entry {}", v.entry);
    }

    for e in &plan.entries {
        let lines = e
            .lines
            .iter()
            .map(|l| {
                let account_id = *by_number
                    .get(&l.account)
                    .ok_or_else(|| anyhow::anyhow!("no account with number '{}'", l.account))?;
                Ok(EntryLineInput {
                    account_id,
                    amount: l.amount_cents,
                    currency: "USD".into(),
                    memo: l.memo.clone(),
                })
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        let id = post_entry_in_tx(
            &mut tx,
            plan.company,
            plan.user,
            PostEntryInput {
                date: e.date,
                memo: e.memo.clone(),
                reference: e.reference.clone(),
                lines,
            },
            JournalEntrySource::Manual,
        )
        .await?;
        println!("posted {} — {} ({id})", e.date, e.memo);
    }

    for r in &plan.reassigns {
        let new_account_id = *by_number
            .get(&r.to_account)
            .ok_or_else(|| anyhow::anyhow!("no account with number '{}'", r.to_account))?;
        let row: Option<(Uuid, Uuid)> =
            sqlx::query_as("SELECT entry_id, account_id FROM journal_lines WHERE id = $1")
                .bind(r.line)
                .fetch_optional(&mut *tx)
                .await?;
        let (entry_id, old_account_id) =
            row.ok_or_else(|| anyhow::anyhow!("no journal line {}", r.line))?;
        anyhow::ensure!(old_account_id != new_account_id, "line {} already on {}", r.line, r.to_account);
        let event = Event::JournalLineReassigned {
            entry_id: entry_id.to_string(),
            line_id: r.line.to_string(),
            old_account_id: old_account_id.to_string(),
            new_account_id: new_account_id.to_string(),
        };
        append_event(&mut tx, plan.company, plan.user, &event).await?;
        println!("reassigned line {} -> {}", r.line, r.to_account);
    }

    tx.commit().await?;
    println!(
        "plan applied atomically: {} voided, {} posted, {} reassigned",
        plan.void.len(),
        plan.entries.len(),
        plan.reassigns.len()
    );
    Ok(())
}
