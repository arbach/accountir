//! Tax-form review CLI: run the app's step-4 review (deterministic per-item
//! enumeration + one focused AI check per item, `tax::review_form_by_id`) over
//! many forms IN PARALLEL and print a consolidated report.
//!   TAXREVIEW_FORMS=<company_uuid>:<form_uuid>[,…]   forms to review
//!   TAXREVIEW_ALL=1                                  or: every form with status='filled'
//!   TAX_REVIEW_MODEL=<model>                         reviewer model (default sonnet)
//!   TAXREVIEW_CONCURRENCY=<n>                        parallel forms (default 5)
//! DATABASE_URL as usual. Read-only apart from the review itself (no status changes).
use sqlx::postgres::PgPoolOptions;
use sqlx::Row;
use uuid::Uuid;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let url = std::env::var("DATABASE_URL")?;
    let model = std::env::var("TAX_REVIEW_MODEL").unwrap_or_else(|_| "sonnet".into());
    let conc: usize = std::env::var("TAXREVIEW_CONCURRENCY").ok().and_then(|s| s.parse().ok()).unwrap_or(5);
    let pool = PgPoolOptions::new().max_connections(8).connect(&url).await?;

    let mut targets: Vec<(Uuid, Uuid, String, String)> = Vec::new(); // (company, form, company_name, code)
    if let Ok(spec) = std::env::var("TAXREVIEW_FORMS") {
        for pair in spec.split(',').filter(|s| !s.trim().is_empty()) {
            let (c, f) = pair
                .split_once(':')
                .ok_or_else(|| anyhow::anyhow!("TAXREVIEW_FORMS entries must be company:form"))?;
            targets.push((c.trim().parse()?, f.trim().parse()?, String::new(), String::new()));
        }
    } else if std::env::var("TAXREVIEW_ALL").is_ok() {
        // tax_forms is FORCE-RLS: it must be read inside a tenant-scoped tx per company.
        let companies = sqlx::query("SELECT id, name FROM companies ORDER BY name")
            .fetch_all(&pool)
            .await?;
        for c in companies {
            let company: Uuid = c.get("id");
            let cname: String = c.get("name");
            let mut tx = pool.begin().await?;
            accountir_cloud::store::event_store::set_tenant(&mut tx, company).await?;
            let rows = sqlx::query(
                "SELECT id, form_code, year FROM tax_forms \
                 WHERE company_id = $1 AND status = 'filled' ORDER BY year, form_code",
            )
            .bind(company)
            .fetch_all(&mut *tx)
            .await?;
            tx.commit().await?;
            for r in rows {
                targets.push((
                    company,
                    r.get("id"),
                    format!("{cname} {}", r.get::<i32, _>("year")),
                    r.get("form_code"),
                ));
            }
        }
    } else {
        anyhow::bail!("set TAXREVIEW_FORMS=company:form,… or TAXREVIEW_ALL=1");
    }
    eprintln!("reviewing {} form(s) with model {model}, {conc} in parallel", targets.len());

    let sem = std::sync::Arc::new(tokio::sync::Semaphore::new(conc));
    let mut handles = Vec::new();
    for (company, form_id, cname, code) in targets {
        let pool = pool.clone();
        let model = model.clone();
        let sem = sem.clone();
        handles.push(tokio::spawn(async move {
            let _permit = sem.acquire_owned().await.expect("semaphore");
            let res = accountir_cloud::tax::review_form_by_id(&pool, company, form_id, &model).await;
            (company, form_id, cname, code, res)
        }));
    }

    let mut total_items = 0usize;
    let mut total_fails = 0usize;
    let mut failed_forms = 0usize;
    for h in handles {
        let (company, form_id, cname, code, res) = h.await?;
        match res {
            Ok(review) => {
                let fails = review.failures();
                total_items += review.verdicts.len();
                total_fails += fails.len();
                let tag = if cname.is_empty() { format!("{company}:{form_id}") } else { format!("{cname} {}", review.form_code) };
                if review.all_ok {
                    println!("PASS {tag} — {} item(s) all ok", review.verdicts.len());
                } else {
                    failed_forms += 1;
                    println!("FAIL {tag} — {}/{} item(s) flagged:", fails.len(), review.verdicts.len());
                    for f in fails {
                        println!("  - {}: {}", f.line, f.note);
                    }
                }
            }
            Err(e) => {
                failed_forms += 1;
                total_fails += 1;
                println!("ERROR {cname} {code} ({company}:{form_id}) — review did not run: {e}");
            }
        }
    }
    println!("---\n{total_items} items checked, {total_fails} flagged, {failed_forms} form(s) not clean");
    Ok(())
}
