//! AI transaction search: translate a user's free-text query ("software
//! subscriptions over $100 last year") into the structured filter the
//! transactions page already understands, via the agent daemon's stateless
//! /oneshot endpoint. The AI only produces filter *parameters* — every value
//! is validated against the company's own accounts/categories here, and the
//! actual data access stays the normal parameterized, RLS-scoped query.

use chrono::NaiveDate;
use std::collections::HashMap;
use uuid::Uuid;

use crate::queries::AccountRow;

pub struct AiTxFilter {
    pub start: Option<NaiveDate>,
    pub end: Option<NaiveDate>,
    pub account_ids: Vec<Uuid>,
    pub source: Option<String>,
    pub direction: Option<String>,
    pub min_cents: Option<i64>,
    pub max_cents: Option<i64>,
    /// OR-matched keywords (rendered into the search box joined with " | ").
    pub keywords: Vec<String>,
    pub vendor: Option<String>,
    pub category: Option<String>,
    pub sort: Option<String>,
    /// One-sentence human description of how the query was interpreted.
    pub note: String,
}

const SYSTEM: &str = "You translate a bookkeeper's natural-language search over a company's \
accounting transactions into a structured filter. You are given today's date, the chart of \
accounts, the known category tags, the known vendor names, and the user's search. \
Reply with ONLY a JSON object (no prose, no code fences) with exactly these keys, using null \
(or [] for the arrays) for any that don't apply: \
\"start\", \"end\" (YYYY-MM-DD dates bounding the period the user asked about; resolve relative \
periods like 'last month' from today's date; null when no period is implied), \
\"account_numbers\" (array of account_number strings from the chart of accounts, ONLY when the \
user clearly scopes to specific accounts — a named bank account, 'rent expense', 'the CC card'), \
\"source\" (one of manual|import|recurring|system|plaid, only if the user asks how entries got \
into the books), \
\"direction\" ('debit' = money INTO the bank/asset account, 'credit' = money OUT of it; only \
when the user clearly means incoming vs outgoing money), \
\"min_amount\", \"max_amount\" (positive dollar amounts bounding the absolute amount), \
\"keywords\" (up to 8 short terms, case-insensitive substring OR-matched against memo, \
reference, vendor name, category and account names; expand a concept into the concrete \
merchant strings likely to appear on bank statements — e.g. 'software subscriptions' -> \
[\"github\",\"adobe\",\"aws\",\"openai\"] — preferring names that appear in the provided vendor \
list; use [] when the other filter keys already capture the search), \
\"vendor\" (a single vendor-name substring, only when the user names one specific vendor/payee), \
\"category\" (an exact value from the category list), \
\"sort\" (date_desc|date_asc|amount_desc|amount_asc; 'largest'/'biggest' -> amount_desc; null \
for the default), \
\"note\" (one short plain-English sentence telling the user how you interpreted their search). \
Never invent account numbers, categories or vendors that are not in the provided lists.";

/// Ask the agent daemon to turn `query` into filter parameters, validating
/// everything it returns against the company's real accounts/categories.
pub async fn translate(
    query: &str,
    today: NaiveDate,
    accounts: &[AccountRow],
    categories: &[String],
    vendors: &[String],
) -> Result<AiTxFilter, String> {
    let coa = accounts
        .iter()
        .map(|a| format!("{} — {} ({})", a.account_number, a.name, a.account_type))
        .collect::<Vec<_>>()
        .join("\n");
    let list_or_none = |v: &[String]| {
        if v.is_empty() { "(none)".to_string() } else { v.join(", ") }
    };
    let prompt = format!(
        "TODAY: {today}\n\nCHART OF ACCOUNTS (account_number — name — type):\n{coa}\n\n\
         CATEGORY TAGS: {cats}\n\nVENDORS: {vends}\n\nUSER SEARCH: {query}\n\n\
         Return the JSON object now.",
        cats = list_or_none(categories),
        vends = list_or_none(vendors),
    );

    let v = crate::doc_ai::oneshot_json(SYSTEM, &prompt).await?;

    let strv = |k: &str| {
        v[k].as_str()
            .map(str::trim)
            .filter(|s| !s.is_empty() && !s.eq_ignore_ascii_case("null"))
            .map(str::to_string)
    };
    let date = |k: &str| strv(k).and_then(|s| NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok());
    // Models sometimes return amounts as strings ("1,500" / "$100") — take both.
    let dollars = |k: &str| {
        v[k].as_f64().or_else(|| {
            v[k].as_str()
                .and_then(|s| s.trim().trim_start_matches('$').replace(',', "").parse::<f64>().ok())
        })
        .filter(|x| *x > 0.0)
    };

    let by_number: HashMap<&str, Uuid> =
        accounts.iter().map(|a| (a.account_number.as_str(), a.id)).collect();
    let account_ids: Vec<Uuid> = v["account_numbers"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|x| x.as_str())
                .filter_map(|n| by_number.get(n.trim()).copied())
                .collect()
        })
        .unwrap_or_default();

    // '|' is the OR separator in the search box — strip it out of terms.
    let keywords: Vec<String> = v["keywords"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|x| x.as_str())
                .map(|s| s.replace('|', " ").trim().to_string())
                .filter(|s| !s.is_empty())
                .take(8)
                .collect()
        })
        .unwrap_or_default();

    Ok(AiTxFilter {
        start: date("start"),
        end: date("end"),
        account_ids,
        source: strv("source")
            .filter(|s| ["manual", "import", "recurring", "system", "plaid"].contains(&s.as_str())),
        direction: strv("direction").filter(|s| s == "debit" || s == "credit"),
        min_cents: dollars("min_amount").map(|x| (x * 100.0).round() as i64),
        max_cents: dollars("max_amount").map(|x| (x * 100.0).round() as i64),
        keywords,
        vendor: strv("vendor"),
        category: strv("category").filter(|c| categories.iter().any(|k| k == c)),
        sort: strv("sort").filter(|s| {
            ["date_desc", "date_asc", "amount_desc", "amount_asc"].contains(&s.as_str())
        }),
        note: strv("note").unwrap_or_else(|| "Set the filters below from your search.".into()),
    })
}
