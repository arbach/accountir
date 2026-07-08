//! Consolidated external-runtime layer for the tax pipeline.
//!
//! Every external call (OpenTax/bridge Python, taxpdf.ts/Deno, review/claude)
//! goes through `run()`, which applies **one** consistent environment
//! (PATH/DENO_DIR/BRIDGE_OUT) and logs every invocation — args, duration, exit
//! code, stderr — to both `tracing` and a persistent JSONL audit log. This kills
//! the scattered, env-fragile `Command::new` sites and makes every pipeline step
//! capturable when something breaks.

use serde_json::{json, Value};
use std::process::Output;
use std::time::Instant;

fn audit_log_path() -> String {
    std::env::var("TAX_AUDIT_LOG")
        .unwrap_or_else(|_| "/var/lib/accountir-cloud/tax-out/tax-pipeline.log".to_string())
}

/// Append one structured line to the persistent tax audit log (best-effort).
pub fn audit(event: &str, fields: Value) {
    let line = json!({ "ts": now_rfc3339(), "event": event, "fields": fields });
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(audit_log_path())
    {
        use std::io::Write;
        let _ = writeln!(f, "{line}");
    }
}

fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

fn base_env(cmd: &mut std::process::Command) {
    let path = std::env::var("PATH").unwrap_or_default();
    cmd.env("PATH", format!("/usr/local/bin:/usr/bin:/bin:{path}"))
        .env(
            "DENO_DIR",
            std::env::var("DENO_DIR").unwrap_or_else(|_| "/var/lib/accountir-cloud/.deno".into()),
        )
        .env(
            "BRIDGE_OUT",
            std::env::var("BRIDGE_OUT").unwrap_or_else(|_| "/var/lib/accountir-cloud/tax-out".into()),
        );
}

/// Run one external tool with the standard env + full logging. `kind` labels the
/// call in logs (e.g. "taxpdf.fill", "compute.bridge"). Returns the raw Output
/// on spawn success (the caller inspects status); Err only on spawn failure.
pub fn run(kind: &str, program: &str, args: &[&str]) -> Result<Output, String> {
    let start = Instant::now();
    tracing::info!(kind, program, ?args, "tax.runtime start");
    let mut cmd = std::process::Command::new(program);
    cmd.args(args);
    base_env(&mut cmd);
    match cmd.output() {
        Ok(o) => {
            let ms = start.elapsed().as_millis() as u64;
            let ok = o.status.success();
            let stderr_tail: String = String::from_utf8_lossy(&o.stderr)
                .lines()
                .filter(|l| !l.trim().is_empty())
                .rev()
                .take(3)
                .collect::<Vec<_>>()
                .join(" | ");
            if ok {
                tracing::info!(kind, ms, "tax.runtime ok");
            } else {
                tracing::error!(kind, ms, code = ?o.status.code(), stderr = %stderr_tail, "tax.runtime FAILED");
            }
            audit(
                "runtime",
                json!({ "kind": kind, "program": program, "ms": ms, "ok": ok,
                        "code": o.status.code(), "stderr": stderr_tail }),
            );
            Ok(o)
        }
        Err(e) => {
            let ms = start.elapsed().as_millis() as u64;
            tracing::error!(kind, ms, error = %e, "tax.runtime spawn failed");
            audit("runtime", json!({ "kind": kind, "program": program, "ms": ms, "ok": false, "error": e.to_string() }));
            Err(format!("{kind} failed to start: {e}"))
        }
    }
}
