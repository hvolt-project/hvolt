// report: the structured half of one in-process command.
//
// `hvolt_run` used to answer with text only, which forced the embedder to parse it back.
// The recorder below collects what the command already knows while it runs: stage
// boundaries with their timings, compiler diagnostics, and the artifacts it wrote.
// `take_json` drains it once per call, so nothing leaks from one command into the next.
use std::sync::Mutex;

use serde_json::{json, Value};

#[derive(Default, Clone)]
struct Stage {
    id: String,
    ms: u128,
    ok: bool,
}

#[derive(Default, Clone)]
struct Diag {
    file: String,
    line: u32,
    col: u32,
    severity: String,
    message: String,
    ambiguous: bool,
}

#[derive(Default, Clone)]
struct Artifact {
    path: String,
    bytes: u64,
}

#[derive(Default)]
struct Report {
    stages: Vec<Stage>,
    diagnostics: Vec<Diag>,
    artifacts: Vec<Artifact>,
}

static REPORT: Mutex<Option<Report>> = Mutex::new(None);

fn with<R>(f: impl FnOnce(&mut Report) -> R) -> R {
    let mut guard = REPORT.lock().unwrap_or_else(|e| e.into_inner());
    if guard.is_none() {
        *guard = Some(Report::default());
    }
    f(guard.as_mut().expect("report initialised"))
}

/// Start a fresh report for the command that is about to run.
pub fn reset() {
    let mut guard = REPORT.lock().unwrap_or_else(|e| e.into_inner());
    *guard = Some(Report::default());
}

/// Record one finished stage. `id` is a stable English identifier; the embedder owns the
/// display name.
pub fn stage(id: &str, ms: u128, ok: bool) {
    with(|r| {
        r.stages.push(Stage {
            id: id.to_string(),
            ms,
            ok,
        })
    })
}

/// Run one stage, record how long it took and whether it succeeded, and hand the result back
/// unchanged. Stages are recorded even when the closure fails, which is what a failed build
/// needs in order to show where it stopped.
pub fn timed<R>(id: &str, f: impl FnOnce() -> Result<R, String>) -> Result<R, String> {
    let start = std::time::Instant::now();
    // Two trace lines bracket every stage: when the embedder process dies mid-build the
    // last line written names the stage that killed it.
    crate::ffi::trace(&format!("stage:begin:{id}"));
    let out = f();
    stage(id, start.elapsed().as_millis(), out.is_ok());
    crate::ffi::trace(&format!(
        "stage:end:{id} ok={} ms={}",
        out.is_ok(),
        start.elapsed().as_millis()
    ));
    out
}

/// Record one compiler diagnostic.
pub fn diagnostic(file: &str, line: u32, col: u32, severity: &str, message: &str, ambiguous: bool) {
    with(|r| {
        r.diagnostics.push(Diag {
            file: file.to_string(),
            line,
            col,
            severity: severity.to_string(),
            message: message.to_string(),
            ambiguous,
        })
    })
}

/// Record one file the command wrote.
pub fn artifact(path: &str, bytes: u64) {
    with(|r| {
        r.artifacts.push(Artifact {
            path: path.to_string(),
            bytes,
        })
    })
}

/// Drain the report into the JSON fragment the FFI envelope embeds.
pub fn take_json() -> Value {
    let mut guard = REPORT.lock().unwrap_or_else(|e| e.into_inner());
    let report = guard.take().unwrap_or_default();
    json!({
        "stages": report
            .stages
            .iter()
            .map(|s| json!({"id": s.id, "ms": s.ms, "ok": s.ok}))
            .collect::<Vec<Value>>(),
        "diagnostics": report
            .diagnostics
            .iter()
            .map(|d| json!({
                "file": d.file,
                "line": d.line,
                "col": d.col,
                "severity": d.severity,
                "message": d.message,
                "ambiguous": d.ambiguous
            }))
            .collect::<Vec<Value>>(),
        "artifacts": report
            .artifacts
            .iter()
            .map(|a| json!({"path": a.path, "bytes": a.bytes}))
            .collect::<Vec<Value>>(),
    })
}
