//! Comprehensive integration tests for the accounting system.
//!
//! These exercise the full event-sourced pipeline (append → project → query) and
//! assert the fundamental accounting invariants that must ALWAYS hold:
//!   * every posted entry is balanced (Σ debits = Σ credits);
//!   * the trial balance is balanced for any set of balanced entries;
//!   * the balance-sheet identity Assets = Liabilities + Equity (incl. net income);
//!   * Net income = Revenue − Expenses;
//!   * voiding/unvoiding and line reassignment preserve those invariants;
//!   * date filtering on reports is correct.
//!
//! Amounts are in integer cents. Debits are positive, credits negative (the
//! engine's `amount` convention).

use accountir::events::types::{Event, EventAccountType, EventEnvelope, JournalLineData};
use accountir::queries::account_queries::AccountQueries;
use accountir::queries::reports::Reports;
use accountir::store::event_store::EventStore;
use accountir::store::migrations::init_schema;
use accountir::store::projections::Projector;
use chrono::NaiveDate;
use rust_decimal::Decimal;

const USER: &str = "test-user";

// ---- test harness -----------------------------------------------------------

fn new_store() -> EventStore {
    let store = EventStore::in_memory().unwrap();
    init_schema(store.connection()).unwrap();
    store
}

/// Append an event and immediately project it into the read model.
fn apply(store: &mut EventStore, event: Event) {
    let stored = store
        .append(EventEnvelope::new(event, USER.to_string()))
        .unwrap();
    let projector = Projector::new(store.connection());
    projector.apply(&stored).unwrap();
}

fn account(store: &mut EventStore, id: &str, ty: EventAccountType, number: &str, name: &str) {
    apply(
        store,
        Event::AccountCreated {
            account_id: id.to_string(),
            account_type: ty,
            account_number: number.to_string(),
            name: name.to_string(),
            parent_id: None,
            currency: Some("USD".to_string()),
            description: None,
        },
    );
}

fn day(y: i32, m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, d).unwrap()
}

/// Post a balanced journal entry. `lines` is (account_id, amount_cents) with
/// debits positive and credits negative; the test fails if they don't net to 0.
fn entry(store: &mut EventStore, id: &str, date: NaiveDate, memo: &str, lines: &[(&str, i64)]) {
    let net: i64 = lines.iter().map(|(_, a)| *a).sum();
    assert_eq!(net, 0, "entry `{id}` is not balanced (nets to {net})");
    let jl: Vec<JournalLineData> = lines
        .iter()
        .enumerate()
        .map(|(i, (acc, amt))| JournalLineData {
            line_id: format!("{id}-l{i}"),
            account_id: acc.to_string(),
            amount: *amt,
            currency: "USD".to_string(),
            exchange_rate: None,
            memo: None,
        })
        .collect();
    apply(
        store,
        Event::JournalEntryPosted {
            entry_id: id.to_string(),
            date,
            memo: memo.to_string(),
            lines: jl,
            reference: None,
            source: None,
        },
    );
}

fn balance(store: &EventStore, account_id: &str, as_of: Option<NaiveDate>) -> i64 {
    AccountQueries::new(store.connection())
        .get_account_balance(account_id, as_of)
        .unwrap()
        .balance
}

/// A small business chart of accounts + a full operating cycle, reused by
/// several tests. Returns the store.
fn business_scenario() -> EventStore {
    let mut s = new_store();
    account(&mut s, "cash", EventAccountType::Asset, "1000", "Cash");
    account(&mut s, "equip", EventAccountType::Asset, "1500", "Equipment");
    account(&mut s, "loan", EventAccountType::Liability, "2000", "Bank Loan");
    account(&mut s, "capital", EventAccountType::Equity, "3000", "Owner Capital");
    account(&mut s, "rev", EventAccountType::Revenue, "4000", "Sales");
    account(&mut s, "exp", EventAccountType::Expense, "5000", "Operating Expense");

    let d = day(2024, 1, 10);
    entry(&mut s, "e1", d, "Owner investment", &[("cash", 100_000), ("capital", -100_000)]);
    entry(&mut s, "e2", d, "Bank loan", &[("cash", 50_000), ("loan", -50_000)]);
    entry(&mut s, "e3", d, "Buy equipment", &[("equip", 30_000), ("cash", -30_000)]);
    entry(&mut s, "e4", d, "Cash sale", &[("cash", 40_000), ("rev", -40_000)]);
    entry(&mut s, "e5", d, "Pay expense", &[("exp", 15_000), ("cash", -15_000)]);
    s
}

// ---- tests ------------------------------------------------------------------

#[test]
fn full_cycle_balances_and_equation_holds() {
    let s = business_scenario();

    // Raw account balances (debit +, credit −).
    assert_eq!(balance(&s, "cash", None), 145_000);
    assert_eq!(balance(&s, "equip", None), 30_000);
    assert_eq!(balance(&s, "loan", None), -50_000);
    assert_eq!(balance(&s, "capital", None), -100_000);
    assert_eq!(balance(&s, "rev", None), -40_000);
    assert_eq!(balance(&s, "exp", None), 15_000);

    let reports = Reports::new(s.connection());

    // Trial balance: debits == credits.
    let tb = reports.trial_balance(None).unwrap();
    assert!(tb.is_balanced, "trial balance must balance");
    assert_eq!(tb.total_debits, tb.total_credits);
    assert_eq!(tb.total_debits, 190_000); // 145k cash + 30k equip + 15k exp

    // Income statement.
    let is = reports
        .income_statement(day(2024, 1, 1), day(2024, 12, 31))
        .unwrap();
    assert_eq!(is.revenue.total, 40_000);
    assert_eq!(is.expenses.total, 15_000);
    assert_eq!(is.net_income, 25_000);

    // Balance sheet: Assets = Liabilities + Equity (incl. net income).
    let bs = reports.balance_sheet(day(2024, 1, 31)).unwrap();
    assert_eq!(bs.total_assets, 175_000); // 145k + 30k
    assert_eq!(bs.liabilities.total, 50_000);
    assert_eq!(bs.equity.total, 125_000); // 100k capital + 25k net income
    assert_eq!(bs.total_liabilities_and_equity, 175_000);
    assert!(bs.is_balanced, "balance sheet identity must hold");
}

#[test]
fn trial_balance_debit_credit_columns_follow_normal_balance() {
    let s = business_scenario();
    let tb = Reports::new(s.connection()).trial_balance(None).unwrap();
    let find = |num: &str| tb.lines.iter().find(|l| l.account_number == num).unwrap();

    // Debit-normal accounts sit in the debit column.
    assert!(find("1000").debit.is_some() && find("1000").credit.is_none()); // asset
    assert!(find("5000").debit.is_some() && find("5000").credit.is_none()); // expense
    // Credit-normal accounts sit in the credit column.
    assert!(find("2000").credit.is_some() && find("2000").debit.is_none()); // liability
    assert!(find("3000").credit.is_some() && find("3000").debit.is_none()); // equity
    assert!(find("4000").credit.is_some() && find("4000").debit.is_none()); // revenue
}

#[test]
fn voiding_an_entry_removes_its_effect() {
    let mut s = business_scenario();

    // Void the expense entry e5.
    apply(
        &mut s,
        Event::JournalEntryVoided {
            entry_id: "e5".to_string(),
            reason: "test void".to_string(),
        },
    );

    assert_eq!(balance(&s, "exp", None), 0, "voided expense disappears");
    assert_eq!(balance(&s, "cash", None), 160_000, "cash no longer reduced");

    let reports = Reports::new(s.connection());
    let tb = reports.trial_balance(None).unwrap();
    assert!(tb.is_balanced, "trial balance still balances after void");

    let is = reports
        .income_statement(day(2024, 1, 1), day(2024, 12, 31))
        .unwrap();
    assert_eq!(is.expenses.total, 0);
    assert_eq!(is.net_income, 40_000);

    let bs = reports.balance_sheet(day(2024, 1, 31)).unwrap();
    assert!(bs.is_balanced);
    assert_eq!(bs.total_assets, 190_000); // 160k cash + 30k equip
}

#[test]
fn unvoiding_restores_the_effect() {
    let mut s = business_scenario();
    apply(
        &mut s,
        Event::JournalEntryVoided {
            entry_id: "e5".to_string(),
            reason: "oops".to_string(),
        },
    );
    apply(
        &mut s,
        Event::JournalEntryUnvoided {
            entry_id: "e5".to_string(),
            reason: "restore".to_string(),
        },
    );

    assert_eq!(balance(&s, "exp", None), 15_000);
    assert_eq!(balance(&s, "cash", None), 145_000);
    let is = Reports::new(s.connection())
        .income_statement(day(2024, 1, 1), day(2024, 12, 31))
        .unwrap();
    assert_eq!(is.net_income, 25_000);
}

#[test]
fn reassigning_a_line_moves_balance_between_accounts() {
    let mut s = new_store();
    account(&mut s, "cash", EventAccountType::Asset, "1000", "Cash");
    account(&mut s, "office", EventAccountType::Expense, "5000", "Office");
    account(&mut s, "travel", EventAccountType::Expense, "5100", "Travel");

    // Debit office, credit cash. Line id will be "rc-l0".
    entry(
        &mut s,
        "rc",
        day(2024, 3, 1),
        "Expense booked to wrong account",
        &[("office", 20_000), ("cash", -20_000)],
    );
    assert_eq!(balance(&s, "office", None), 20_000);
    assert_eq!(balance(&s, "travel", None), 0);

    // Reclassify the office line to travel.
    apply(
        &mut s,
        Event::JournalLineReassigned {
            entry_id: "rc".to_string(),
            line_id: "rc-l0".to_string(),
            old_account_id: "office".to_string(),
            new_account_id: "travel".to_string(),
        },
    );

    assert_eq!(balance(&s, "office", None), 0, "moved off office");
    assert_eq!(balance(&s, "travel", None), 20_000, "now on travel");
    assert_eq!(balance(&s, "cash", None), -20_000, "cash unchanged");
    assert!(Reports::new(s.connection())
        .trial_balance(None)
        .unwrap()
        .is_balanced);
}

#[test]
fn as_of_date_filters_balances_and_reports() {
    let mut s = new_store();
    account(&mut s, "cash", EventAccountType::Asset, "1000", "Cash");
    account(&mut s, "rev", EventAccountType::Revenue, "4000", "Sales");

    entry(&mut s, "jan", day(2024, 1, 15), "Jan sale", &[("cash", 10_000), ("rev", -10_000)]);
    entry(&mut s, "feb", day(2024, 2, 15), "Feb sale", &[("cash", 20_000), ("rev", -20_000)]);

    // Point-in-time balances.
    assert_eq!(balance(&s, "cash", Some(day(2024, 1, 31))), 10_000);
    assert_eq!(balance(&s, "cash", Some(day(2024, 2, 28))), 30_000);
    // An entry posted exactly on the as-of date is included.
    assert_eq!(balance(&s, "cash", Some(day(2024, 1, 15))), 10_000);

    let reports = Reports::new(s.connection());
    assert_eq!(
        reports
            .income_statement(day(2024, 1, 1), day(2024, 1, 31))
            .unwrap()
            .revenue
            .total,
        10_000,
        "January P&L only sees the January sale"
    );
    assert_eq!(
        reports
            .income_statement(day(2024, 2, 1), day(2024, 2, 28))
            .unwrap()
            .revenue
            .total,
        20_000,
        "February P&L only sees the February sale"
    );
    assert_eq!(
        reports
            .income_statement(day(2024, 1, 1), day(2024, 12, 31))
            .unwrap()
            .revenue
            .total,
        30_000,
        "full-year P&L sees both"
    );
}

#[test]
fn zero_balance_accounts_are_excluded_from_trial_balance() {
    let mut s = new_store();
    account(&mut s, "cash", EventAccountType::Asset, "1000", "Cash");
    account(&mut s, "susp", EventAccountType::Asset, "1999", "Suspense");
    account(&mut s, "rev", EventAccountType::Revenue, "4000", "Sales");

    // Route money through suspense, then clear it back to zero.
    entry(&mut s, "p1", day(2024, 1, 1), "into suspense", &[("susp", 5_000), ("rev", -5_000)]);
    entry(&mut s, "p2", day(2024, 1, 2), "clear suspense", &[("cash", 5_000), ("susp", -5_000)]);

    assert_eq!(balance(&s, "susp", None), 0);
    let tb = Reports::new(s.connection()).trial_balance(None).unwrap();
    assert!(
        tb.lines.iter().all(|l| l.account_number != "1999"),
        "zero-balance suspense account must not appear on the trial balance"
    );
    assert!(tb.is_balanced);
}

#[test]
fn multi_line_split_entry_is_handled() {
    let mut s = new_store();
    account(&mut s, "cash", EventAccountType::Asset, "1000", "Cash");
    account(&mut s, "a", EventAccountType::Expense, "5000", "Expense A");
    account(&mut s, "b", EventAccountType::Expense, "5100", "Expense B");

    // One credit split across two debits.
    entry(
        &mut s,
        "split",
        day(2024, 4, 1),
        "Split expense",
        &[("a", 6_000), ("b", 4_000), ("cash", -10_000)],
    );

    assert_eq!(balance(&s, "a", None), 6_000);
    assert_eq!(balance(&s, "b", None), 4_000);
    assert_eq!(balance(&s, "cash", None), -10_000);
    let is = Reports::new(s.connection())
        .income_statement(day(2024, 1, 1), day(2024, 12, 31))
        .unwrap();
    assert_eq!(is.expenses.total, 10_000);
}

#[test]
fn foreign_currency_line_round_trips() {
    let mut s = new_store();
    account(&mut s, "wise", EventAccountType::Asset, "1010", "Wise EUR");
    account(&mut s, "exp", EventAccountType::Expense, "5000", "Contractor");

    apply(
        &mut s,
        Event::JournalEntryPosted {
            entry_id: "fx".to_string(),
            date: day(2024, 5, 1),
            memo: "EUR contractor payment".to_string(),
            lines: vec![
                JournalLineData {
                    line_id: "fx-l0".to_string(),
                    account_id: "exp".to_string(),
                    amount: 50_000,
                    currency: "EUR".to_string(),
                    exchange_rate: Some(Decimal::new(108, 2)), // 1.08
                    memo: None,
                },
                JournalLineData {
                    line_id: "fx-l1".to_string(),
                    account_id: "wise".to_string(),
                    amount: -50_000,
                    currency: "EUR".to_string(),
                    exchange_rate: Some(Decimal::new(108, 2)),
                    memo: None,
                },
            ],
            reference: None,
            source: None,
        },
    );

    // Currency persists on the line through the projection.
    let currency: String = s
        .connection()
        .query_row(
            "SELECT currency FROM journal_lines WHERE id = 'fx-l0'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(currency, "EUR");
    assert_eq!(balance(&s, "exp", None), 50_000);
}

/// Property-style check: ANY collection of balanced two-line entries must leave
/// the trial balance and balance sheet balanced. Uses a deterministic LCG so the
/// test is reproducible.
#[test]
fn invariant_many_random_balanced_entries_keep_books_balanced() {
    let mut s = new_store();
    let accounts = [
        ("cash", EventAccountType::Asset, "1000"),
        ("ar", EventAccountType::Asset, "1100"),
        ("ap", EventAccountType::Liability, "2000"),
        ("equity", EventAccountType::Equity, "3000"),
        ("rev", EventAccountType::Revenue, "4000"),
        ("exp", EventAccountType::Expense, "5000"),
    ];
    for (id, ty, num) in accounts.iter() {
        account(&mut s, id, ty.clone(), num, id);
    }

    let mut seed: u64 = 0x9E3779B97F4A7C15;
    let mut next = |bound: u64| -> u64 {
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (seed >> 33) % bound
    };

    for i in 0..200 {
        let a = next(accounts.len() as u64) as usize;
        let mut b = next(accounts.len() as u64) as usize;
        if b == a {
            b = (b + 1) % accounts.len();
        }
        let amount = (next(9_900) + 100) as i64; // 100..=9999 cents
        entry(
            &mut s,
            &format!("r{i}"),
            day(2024, 1, 1),
            "random",
            &[(accounts[a].0, amount), (accounts[b].0, -amount)],
        );

        // Invariant must hold after every single entry.
        let tb = Reports::new(s.connection()).trial_balance(None).unwrap();
        assert!(
            tb.is_balanced && tb.total_debits == tb.total_credits,
            "trial balance unbalanced after entry r{i}"
        );
    }

    let bs = Reports::new(s.connection())
        .balance_sheet(day(2024, 12, 31))
        .unwrap();
    assert!(
        bs.is_balanced,
        "balance sheet identity broken: assets {} vs L+E {}",
        bs.total_assets, bs.total_liabilities_and_equity
    );
}
