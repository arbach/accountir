//! accountir-agentd — maintains one persistent Claude CLI session per company.
//!
//! Each company gets a long-lived `claude -p --input/output-format stream-json`
//! child process. Turns are serialized per company; idle children are reaped and
//! transparently resumed later via `--resume <session-id>` (transcripts persist on
//! disk), so a company's conversation context survives reaps, daemon restarts,
//! and reboots. Sessions map to companies via the `agent_sessions` table.
//!
//! Tool surface: built-ins disabled; only the company-scoped `accounting` MCP
//! server (served by accountir-cloud, bearer-token auth) is allowed.
//!
//! HTTP API (loopback only):
//!   POST /turn  {company_id, user_id?, message}  -> {ok, events: [stream-json events]}
//!   POST /reset {company_id}                     -> {ok}   (forgets the session)
//!   GET  /health

use std::{
    collections::HashMap,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

use axum::{
    body::{Body, Bytes},
    extract::State,
    response::Response,
    routing::{get, post},
    Json, Router,
};
use tokio_stream::wrappers::ReceiverStream;
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::PgPool;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::Mutex;
use uuid::Uuid;

const SYSTEM_PROMPT: &str = r#"You are the dedicated AI accountant for one company in Accountir, a double-entry accounting system. You have a persistent, long-running session for this company: remember context across conversations and build on prior work.

You manage the books through your `accounting` tools (chart of accounts, journal entries, reports, bank data). Always:
- Confirm before posting or modifying entries unless the user unambiguously asked you to act now.
- Remember: positive amounts are debits, negative are credits; entry lines must sum to zero.
- Format money as dollars (e.g. $100.00) when speaking; pass dollar amounts to tools.
- Be concise and bookkeeping-precise. If a tool errors, explain plainly and propose a fix.
- You can only access THIS company's data; never speculate about other companies.
- You can drive the app's UI: use navigate_to_page to take the user to a page (e.g. after running a report, offer to open it on screen).
- You can search the web (WebSearch) — especially useful for identifying unknown merchants or cryptic bank memo strings when categorizing transactions. Never include the company's financial data in search queries; search only for the merchant/payee name."#;

struct AgentProc {
    child: Child,
    stdin: ChildStdin,
    stdout: tokio::io::Lines<BufReader<ChildStdout>>,
    last_used: Instant,
}

#[derive(Clone)]
struct Cfg {
    mcp_url: String,
    model: String,
    state_dir: PathBuf,
    claude_bin: String,
    idle_secs: u64,
    turn_timeout_secs: u64,
}

struct Daemon {
    pool: PgPool,
    cfg: Cfg,
    agents: Mutex<HashMap<Uuid, Arc<Mutex<Option<AgentProc>>>>>,
}

fn random_token() -> String {
    use rand::RngCore;
    let mut b = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut b);
    hex::encode(b)
}

/// Load (or create) the durable session row for a company.
async fn ensure_session_row(
    pool: &PgPool,
    company_id: Uuid,
) -> anyhow::Result<(Uuid, String, bool)> {
    if let Some((sid, tok)) = sqlx::query_as::<_, (Uuid, String)>(
        "SELECT session_id, mcp_token FROM agent_sessions WHERE company_id = $1",
    )
    .bind(company_id)
    .fetch_optional(pool)
    .await?
    {
        return Ok((sid, tok, true));
    }
    let sid = Uuid::new_v4();
    let tok = random_token();
    sqlx::query("INSERT INTO agent_sessions (company_id, session_id, mcp_token) VALUES ($1,$2,$3)")
        .bind(company_id)
        .bind(sid)
        .bind(&tok)
        .execute(pool)
        .await?;
    Ok((sid, tok, false))
}

async fn spawn_proc(
    cfg: &Cfg,
    company_id: Uuid,
    session_id: Uuid,
    token: &str,
    resume: bool,
) -> anyhow::Result<AgentProc> {
    let cwd = cfg.state_dir.join(company_id.to_string());
    tokio::fs::create_dir_all(&cwd).await?;
    let mcp_path = cwd.join("mcp.json");
    let mcp_json = json!({
        "mcpServers": {
            "accounting": {
                "type": "http",
                "url": cfg.mcp_url,
                "headers": { "Authorization": format!("Bearer {token}") }
            }
        }
    });
    tokio::fs::write(&mcp_path, serde_json::to_vec_pretty(&mcp_json)?).await?;

    let mut cmd = Command::new(&cfg.claude_bin);
    cmd.arg("-p")
        .arg("--input-format")
        .arg("stream-json")
        .arg("--output-format")
        .arg("stream-json")
        .arg("--verbose")
        .arg("--include-partial-messages")
        .arg("--strict-mcp-config")
        .arg("--mcp-config")
        .arg(&mcp_path)
        .arg("--permission-mode")
        .arg("dontAsk")
        .arg("--allowedTools")
        .arg("mcp__accounting__*,WebSearch")
        .arg("--disallowedTools")
        .args([
            "Bash", "Edit", "Write", "Read", "Glob", "Grep", "WebFetch",
            "NotebookEdit", "Task", "Agent", "Skill", "TodoWrite", "EnterPlanMode",
            "ExitPlanMode", "Workflow", "ToolSearch", "KillShell", "BashOutput",
        ])
        .arg("--tools")
        .arg("WebSearch")
        .arg("--system-prompt")
        .arg(SYSTEM_PROMPT)
        .arg("--model")
        .arg(&cfg.model)
        .current_dir(&cwd)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true);
    if resume {
        cmd.arg("--resume").arg(session_id.to_string());
    } else {
        cmd.arg("--session-id").arg(session_id.to_string());
    }
    let mut child = cmd.spawn()?;
    let stdin = child.stdin.take().expect("piped stdin");
    let stdout = BufReader::new(child.stdout.take().expect("piped stdout")).lines();
    tracing::info!(company = %company_id, session = %session_id, resume, "spawned claude agent");
    Ok(AgentProc {
        child,
        stdin,
        stdout,
        last_used: Instant::now(),
    })
}

fn user_turn_line(message: &str) -> anyhow::Result<String> {
    Ok(serde_json::to_string(&json!({
        "type": "user",
        "message": { "role": "user", "content": [{ "type": "text", "text": message }] }
    }))? + "\n")
}

/// Run one turn against the (possibly fresh) agent process, forwarding every
/// stream-json event line into `tx` AS IT ARRIVES, through the terminating
/// `result` event.
async fn run_turn(
    d: &Daemon,
    slot: &mut Option<AgentProc>,
    company_id: Uuid,
    message: &str,
    tx: &tokio::sync::mpsc::Sender<Result<Bytes, std::io::Error>>,
) -> anyhow::Result<()> {
    let line = user_turn_line(message)?;

    if slot.is_none() {
        let (sid, tok, existing) = ensure_session_row(&d.pool, company_id).await?;
        *slot = Some(spawn_proc(&d.cfg, company_id, sid, &tok, existing).await?);
    }

    // Write the turn; if the idle child died since last use, respawn-resume once.
    {
        let proc = slot.as_mut().unwrap();
        if proc.stdin.write_all(line.as_bytes()).await.is_err()
            || proc.stdin.flush().await.is_err()
        {
            tracing::warn!(company = %company_id, "agent stdin closed; respawning with resume");
            *slot = None;
            let (sid, tok, _) = ensure_session_row(&d.pool, company_id).await?;
            *slot = Some(spawn_proc(&d.cfg, company_id, sid, &tok, true).await?);
            let proc = slot.as_mut().unwrap();
            proc.stdin.write_all(line.as_bytes()).await?;
            proc.stdin.flush().await?;
        }
    }

    let proc = slot.as_mut().unwrap();
    let mut forwarded = 0usize;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(d.cfg.turn_timeout_secs);
    loop {
        match tokio::time::timeout_at(deadline, proc.stdout.next_line()).await {
            Err(_) => anyhow::bail!("turn timed out after {}s", d.cfg.turn_timeout_secs),
            Ok(Ok(Some(l))) => {
                let Ok(v) = serde_json::from_str::<Value>(&l) else { continue };
                let done = v.get("type").and_then(|t| t.as_str()) == Some("result");
                forwarded += 1;
                // Forward the raw line immediately; receiver gone = client hung up,
                // but we still drain to the result so the session stays consistent.
                let _ = tx.send(Ok(Bytes::from(l + "\n"))).await;
                if done {
                    break;
                }
            }
            Ok(Ok(None)) | Ok(Err(_)) => {
                // Stream closed mid-turn. If we got nothing at all, the stored
                // session likely failed to resume — drop it so the next turn
                // starts a fresh session instead of failing forever.
                if forwarded == 0 {
                    let _ = sqlx::query("DELETE FROM agent_sessions WHERE company_id = $1")
                        .bind(company_id)
                        .execute(&d.pool)
                        .await;
                }
                anyhow::bail!("agent process closed stream mid-turn ({forwarded} events)");
            }
        }
    }
    proc.last_used = Instant::now();
    Ok(())
}

#[derive(Deserialize)]
struct TurnReq {
    company_id: Uuid,
    user_id: Option<Uuid>,
    message: String,
}

async fn turn(State(d): State<Arc<Daemon>>, Json(req): Json<TurnReq>) -> Response {
    let (tx, rx) = tokio::sync::mpsc::channel::<Result<Bytes, std::io::Error>>(64);
    tokio::spawn(async move {
        let slot = {
            let mut m = d.agents.lock().await;
            m.entry(req.company_id)
                .or_insert_with(|| Arc::new(Mutex::new(None)))
                .clone()
        };
        // Per-company mutex serializes turns; other companies proceed in parallel.
        let mut guard = slot.lock().await;

        // Attribute MCP tool calls to the requesting user.
        if let Some(uid) = req.user_id {
            // Row may not exist yet on the very first turn; run_turn ensures it,
            // so set attribution both before (best-effort) and rely on it below.
            let _ = sqlx::query(
                "UPDATE agent_sessions SET last_user_id = $2, updated_at = now() WHERE company_id = $1",
            )
            .bind(req.company_id)
            .bind(uid)
            .execute(&d.pool)
            .await;
        }

        if let Err(e) = run_turn(&d, &mut guard, req.company_id, &req.message, &tx).await {
            *guard = None; // kill_on_drop reaps a wedged child
            tracing::error!(company = %req.company_id, error = %e, "turn failed");
            let line = json!({ "type": "daemon_error", "error": e.to_string() }).to_string() + "\n";
            let _ = tx.send(Ok(Bytes::from(line))).await;
        }
        if let Some(uid) = req.user_id {
            let _ = sqlx::query(
                "UPDATE agent_sessions SET last_user_id = $2, updated_at = now() WHERE company_id = $1",
            )
            .bind(req.company_id)
            .bind(uid)
            .execute(&d.pool)
            .await;
        }
    });
    Response::builder()
        .header("content-type", "application/x-ndjson")
        .body(Body::from_stream(ReceiverStream::new(rx)))
        .unwrap()
}

#[derive(Deserialize)]
struct OneshotReq {
    prompt: String,
    system: Option<String>,
    model: Option<String>,
}

/// Stateless one-shot completion (no session, no tools, no MCP). Used for
/// batch text work like statement parsing, on the same subscription auth.
async fn oneshot(State(d): State<Arc<Daemon>>, Json(req): Json<OneshotReq>) -> Json<Value> {
    let mut cmd = Command::new(&d.cfg.claude_bin);
    cmd.arg("-p")
        .arg("--output-format")
        .arg("json")
        .arg("--strict-mcp-config")
        .arg("--tools")
        .arg("")
        .arg("--disallowedTools")
        .args(["Bash", "Edit", "Write", "Read", "Glob", "Grep", "WebFetch", "WebSearch"])
        .arg("--permission-mode")
        .arg("dontAsk")
        .arg("--model")
        .arg(req.model.as_deref().unwrap_or(&d.cfg.model))
        .current_dir(&d.cfg.state_dir)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true);
    if let Some(s) = &req.system {
        cmd.arg("--system-prompt").arg(s);
    }
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => return Json(json!({ "ok": false, "error": format!("spawn: {e}") })),
    };
    if let Some(mut stdin) = child.stdin.take() {
        if stdin.write_all(req.prompt.as_bytes()).await.is_err() {
            return Json(json!({ "ok": false, "error": "failed to write prompt" }));
        }
        // dropping stdin closes it; claude treats the piped text as the prompt
    }
    let out = match tokio::time::timeout(
        Duration::from_secs(d.cfg.turn_timeout_secs),
        child.wait_with_output(),
    )
    .await
    {
        Err(_) => return Json(json!({ "ok": false, "error": "oneshot timed out" })),
        Ok(Err(e)) => return Json(json!({ "ok": false, "error": format!("wait: {e}") })),
        Ok(Ok(o)) => o,
    };
    let parsed: Value = match serde_json::from_slice(&out.stdout) {
        Ok(v) => v,
        Err(e) => {
            return Json(json!({
                "ok": false,
                "error": format!("bad CLI output ({e}); exit={:?}", out.status.code())
            }))
        }
    };
    let is_error = parsed.get("is_error").and_then(|v| v.as_bool()).unwrap_or(false);
    let result = parsed.get("result").and_then(|v| v.as_str()).unwrap_or("").to_string();
    tracing::info!(
        chars_in = req.prompt.len(),
        chars_out = result.len(),
        cost = parsed.get("total_cost_usd").and_then(|v| v.as_f64()).unwrap_or(0.0),
        "oneshot completed"
    );
    Json(json!({ "ok": !is_error && !result.is_empty(), "result": result }))
}

#[derive(Deserialize)]
struct ResetReq {
    company_id: Uuid,
}

async fn reset(State(d): State<Arc<Daemon>>, Json(req): Json<ResetReq>) -> Json<Value> {
    let slot = {
        let mut m = d.agents.lock().await;
        m.entry(req.company_id)
            .or_insert_with(|| Arc::new(Mutex::new(None)))
            .clone()
    };
    let mut guard = slot.lock().await;
    *guard = None;
    let _ = sqlx::query("DELETE FROM agent_sessions WHERE company_id = $1")
        .bind(req.company_id)
        .execute(&d.pool)
        .await;
    tracing::info!(company = %req.company_id, "agent session reset");
    Json(json!({ "ok": true }))
}

async fn health() -> &'static str {
    "ok"
}

async fn reaper(d: Arc<Daemon>) {
    loop {
        tokio::time::sleep(Duration::from_secs(60)).await;
        let snapshot: Vec<(Uuid, Arc<Mutex<Option<AgentProc>>>)> = {
            let m = d.agents.lock().await;
            m.iter().map(|(k, v)| (*k, v.clone())).collect()
        };
        for (cid, slot) in snapshot {
            if let Ok(mut g) = slot.try_lock() {
                let idle = g
                    .as_ref()
                    .map(|p| p.last_used.elapsed() > Duration::from_secs(d.cfg.idle_secs))
                    .unwrap_or(false);
                if idle {
                    *g = None;
                    tracing::info!(company = %cid, "reaped idle agent (will resume on next turn)");
                }
            }
        }
    }
}

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).ok().filter(|s| !s.is_empty()).unwrap_or_else(|| default.to_string())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "agentd=info".into()),
        )
        .init();

    let database_url = std::env::var("DATABASE_URL")
        .map_err(|_| anyhow::anyhow!("DATABASE_URL must be set"))?;
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(4)
        .connect(&database_url)
        .await?;

    let home = std::env::var("HOME").unwrap_or_else(|_| "/home/ubuntu".into());
    let cfg = Cfg {
        mcp_url: env_or("AGENT_MCP_URL", "http://127.0.0.1:9877/mcp"),
        model: env_or("AGENT_MODEL", "sonnet"),
        state_dir: PathBuf::from(env_or("AGENT_STATE_DIR", &format!("{home}/agentd-state"))),
        claude_bin: env_or("CLAUDE_BIN", &format!("{home}/.local/bin/claude")),
        idle_secs: env_or("AGENT_IDLE_SECS", "1800").parse().unwrap_or(1800),
        turn_timeout_secs: env_or("AGENT_TURN_TIMEOUT_SECS", "600").parse().unwrap_or(600),
    };
    tokio::fs::create_dir_all(&cfg.state_dir).await?;

    let d = Arc::new(Daemon {
        pool,
        cfg,
        agents: Mutex::new(HashMap::new()),
    });
    tokio::spawn(reaper(d.clone()));

    let bind = env_or("AGENTD_BIND", "127.0.0.1:9878");
    let app = Router::new()
        .route("/turn", post(turn))
        .route("/reset", post(reset))
        .route("/oneshot", post(oneshot))
        .route("/health", get(health))
        .with_state(d);
    tracing::info!(%bind, "starting accountir-agentd");
    let listener = tokio::net::TcpListener::bind(&bind).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
