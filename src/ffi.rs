// ffi: the C surface the on-device IDE loads. One entry, one shape:
//
//   char *hvolt_run(const char *argv_json, const char *cwd);
//
// `argv_json` is a JSON array of strings (the arguments after the program
// name), `cwd` is where the command runs (NULL = leave the process cwd alone).
// The returned pointer is a JSON object `{"code": i32, "output": "..."}` that
// the caller must release with [`hvolt_free`].
//
// Everything the command prints is captured: stdout and stderr are redirected
// to a temporary file for the duration of the call, so the embedder gets the
// same text the CLI would have shown.
// This module *is* the C boundary: every entry here is `unsafe extern "C"` by contract, so the
// crate-wide `unsafe_code = "warn"` gate is waived for it alone.
#![allow(unsafe_code)]

use std::ffi::{CStr, CString};
use std::os::raw::c_char;

/// Append one line to the trace file (`$HVOLT_TRACE`, else `$HOME/hvolt-trace.log`).
///
/// The embedder redirects stdio into a buffer that a crash throws away, so the
/// only way to see how far a command got is a file written line by line.
pub fn trace(msg: &str) {
    #[cfg(feature = "ohos")]
    {
        use std::io::Write;
        let path = std::env::var("HVOLT_TRACE").unwrap_or_else(|_| {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
            format!("{home}/hvolt-trace.log")
        });
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
        {
            let _ = writeln!(f, "{msg}");
        }
    }
    #[cfg(not(feature = "ohos"))]
    {
        let _ = msg;
    }
}

/// Run one hvolt command in-process and return its captured result.
///
/// # Safety
/// Both pointers must be valid NUL-terminated strings (or NULL); the caller
/// owns the result and must free it with [`hvolt_free`].
#[no_mangle]
pub unsafe extern "C" fn hvolt_run(argv_json: *const c_char, cwd: *const c_char) -> *mut c_char {
    let argv = if argv_json.is_null() {
        Vec::new()
    } else {
        match serde_json::from_str::<Vec<String>>(
            CStr::from_ptr(argv_json).to_str().unwrap_or("[]"),
        ) {
            Ok(v) => v,
            Err(e) => return leak(result_json(-1, &format!("bad argv json: {e}"), &[])),
        }
    };

    let cwd = if cwd.is_null() {
        None
    } else {
        Some(CStr::from_ptr(cwd).to_string_lossy().to_string())
    };

    crate::ffi::report::reset();
    install_panic_trace();
    let (code, output) = run_captured(&argv, cwd.as_deref());
    crate::ffi::trace(&format!("ffi:captured code={code} bytes={}", output.len()));
    let json = result_json(code, &output, &argv);
    crate::ffi::trace(&format!("ffi:json bytes={}", json.len()));
    leak(json)
}

/// Free a string returned by [`hvolt_run`].
///
/// # Safety
/// The pointer must come from [`hvolt_run`] and must not be used afterwards.
#[no_mangle]
pub unsafe extern "C" fn hvolt_free(ptr: *mut c_char) {
    if !ptr.is_null() {
        drop(CString::from_raw(ptr));
    }
}

/// The library version, for a load-time probe that proves the symbols resolved.
#[no_mangle]
pub extern "C" fn hvolt_lib_version() -> *const c_char {
    concat!(env!("CARGO_PKG_VERSION"), "\0").as_ptr() as *const c_char
}

fn result_json(code: i32, output: &str, argv: &[String]) -> String {
    let report = crate::ffi::report::take_json();
    serde_json::json!({
        "code": code,
        "argv": argv,
        "output": output,
        "stages": report["stages"],
        "diagnostics": report["diagnostics"],
        "artifacts": report["artifacts"],
    })
    .to_string()
}

#[allow(dead_code)]
fn escape_json(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn leak(s: String) -> *mut c_char {
    match CString::new(s) {
        Ok(c) => c.into_raw(),
        Err(_) => CString::new("{\"code\":-1,\"output\":\"nul byte in result\"}")
            .expect("static has no NUL")
            .into_raw(),
    }
}

/// Start a log follower; returns a handle as a JSON string.
///
/// # Safety
///
/// `params_json` must point to a NUL-terminated C string that stays valid for the duration of the
/// call. The returned pointer is owned by the caller and must be released with `hvolt_free`.
#[no_mangle]
pub unsafe extern "C" fn hvolt_stream_start(params_json: *const c_char) -> *mut c_char {
    let params = CStr::from_ptr(params_json).to_str().unwrap_or("");
    match crate::device::stream::start(params) {
        Ok(handle) => leak(format!("{{\"code\":0,\"handle\":{handle}}}")),
        Err(e) => leak(format!("{{\"code\":-1,\"output\":{}}}", escape_json(&e))),
    }
}

/// Wait for lines newer than `from` (up to `wait_ms`) and return them as `{next, rows}`.
///
/// # Safety
///
/// No pointer is dereferenced. The caller must pass a `handle` from `hvolt_stream_start` and must
/// not use it after `hvolt_stream_stop`. The returned pointer is released with `hvolt_free`.
#[no_mangle]
pub unsafe extern "C" fn hvolt_stream_next(
    handle: u64,
    from: u64,
    max: u32,
    wait_ms: u32,
) -> *mut c_char {
    match crate::device::stream::next(handle, from, max as usize, wait_ms as u64) {
        Ok(json) => leak(json),
        Err(e) => leak(format!("{{\"code\":-1,\"output\":{}}}", escape_json(&e))),
    }
}

/// Stop a follower and release its ring.
///
/// # Safety
///
/// No pointer is dereferenced. `handle` must be the value returned by `hvolt_stream_start` and
/// must not be stopped twice.
#[no_mangle]
pub unsafe extern "C" fn hvolt_stream_stop(handle: u64) {
    crate::device::stream::stop(handle);
}

/// Redirect stdout/stderr into a temp file, run the command, restore, return text.
/// A panic inside a command unwinds to the boundary, but a *second* panic while the first is
/// in flight aborts the process and the embedder gets nothing back. This hook writes the
/// message and location to the trace file first, so the next launch can read what killed it.
fn install_panic_trace() {
    use std::sync::Once;
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let msg = info
                .payload()
                .downcast_ref::<&str>()
                .map(|s| s.to_string())
                .or_else(|| info.payload().downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "non-string panic".to_string());
            let at = info
                .location()
                .map(|l| format!("{}:{}", l.file(), l.line()))
                .unwrap_or_else(|| "unknown".to_string());
            crate::ffi::trace(&format!("panic:{msg} at {at}"));
            previous(info);
        }));
    });
}

fn run_captured(argv: &[String], cwd: Option<&str>) -> (i32, String) {
    let saved_cwd = std::env::current_dir().ok();
    if let Some(dir) = cwd {
        if let Err(e) = std::env::set_current_dir(dir) {
            return (-1, format!("chdir {dir}: {e}"));
        }
    }

    let redir = crate::ffi::redirect::Capture::start();
    // A panic in a command must not abort the host app: it unwinds to here and
    // becomes an ordinary failure result (the `ohos` profile builds with unwind).
    let code =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| dispatch(argv))).unwrap_or(-1);
    let output = redir.map(|c| c.finish()).unwrap_or_default();

    if let Some(dir) = saved_cwd {
        let _ = std::env::set_current_dir(dir);
    }
    (code, output)
}

fn dispatch(argv: &[String]) -> i32 {
    use clap::Parser;
    let mut full = vec!["hvolt".to_string()];
    full.extend(argv.iter().cloned());
    match crate::support::cli::Args::try_parse_from(full) {
        Ok(args) => {
            crate::cli::dispatch(args.command);
            0
        }
        Err(e) => {
            // clap prints help/usage to stdout on --help and to stderr on error.
            print!("{e}");
            e.exit_code()
        }
    }
}

/// stdout/stderr redirection into a temp file (unix-only; the embedder target).
pub mod redirect; // implementation lives in redirect.rs
pub mod report; // structured stages, diagnostics and artifacts

#[cfg(test)]
mod tests {
    use std::ffi::{CStr, CString};

    // The envelope is the contract the embedder reads: every key it relies on has to be
    // there on a successful call, and a second call must not see the first one's stages.
    #[test]
    fn envelope_carries_report_fields() {
        let argv = CString::new("[\"--version\"]").expect("argv");
        let ptr = unsafe { super::hvolt_run(argv.as_ptr(), std::ptr::null()) };
        let text = unsafe { CStr::from_ptr(ptr) }.to_string_lossy().to_string();
        unsafe { super::hvolt_free(ptr) };
        let value: serde_json::Value = serde_json::from_str(&text).expect("json");
        assert_eq!(value["code"], 0);
        assert!(value["argv"].is_array());
        assert!(value["output"].is_string());
        assert!(value["stages"].is_array());
        assert!(value["diagnostics"].is_array());
        assert!(value["artifacts"].is_array());
    }
    // Needs the SDK and a buildable sample on the machine; run it explicitly with
    // `cargo test --release -- --ignored`.
    #[test]
    #[ignore]
    fn a_build_reports_its_stages() {
        let sample = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/hello-world");
        assert!(sample.is_dir(), "sample project missing");
        let argv = CString::new("[\"build\"]").expect("argv");
        let cwd = CString::new(sample.to_string_lossy().to_string()).expect("cwd");
        let ptr = unsafe { super::hvolt_run(argv.as_ptr(), cwd.as_ptr()) };
        let text = unsafe { CStr::from_ptr(ptr) }.to_string_lossy().to_string();
        unsafe { super::hvolt_free(ptr) };
        let value: serde_json::Value = serde_json::from_str(&text).expect("json");
        assert_eq!(value["code"], 0, "build failed: {text}");
        let ids: Vec<String> = value["stages"]
            .as_array()
            .expect("stages")
            .iter()
            .map(|s| s["id"].as_str().unwrap_or_default().to_string())
            .collect();
        for want in ["resolve", "merge", "pack"] {
            assert!(
                ids.iter().any(|i| i == want),
                "missing stage {want} in {ids:?}"
            );
        }
        assert!(
            value["stages"]
                .as_array()
                .unwrap()
                .iter()
                .all(|s| s["ms"].as_u64().is_some() && s["ok"].as_bool() == Some(true)),
            "stage timings or outcomes missing: {}",
            value["stages"]
        );
    }

    // Needs the SDK; run with `cargo test --release -- --ignored`.
    #[test]
    #[ignore]
    fn a_broken_source_reports_a_diagnostic() {
        let sample = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/hello-world");
        let tmp = std::env::temp_dir().join("hvolt-diag-sample");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).expect("mkdir");
        for entry in ["hvolt.toml", "src", "resources"].iter() {
            let from = sample.join(entry);
            if from.is_file() {
                let _ = std::fs::copy(&from, tmp.join(entry));
            } else if from.is_dir() {
                let _ = std::fs::remove_dir_all(tmp.join(entry));
                std::fs::create_dir_all(tmp.join(entry)).expect("mkdir child");
                copy_tree(&from, &tmp.join(entry)).expect("copy tree");
            }
        }
        let page = tmp.join("src/pages/Index.ets");
        let mut text = std::fs::read_to_string(&page).expect("read page");
        text.push_str("\nconst broken: number = ;\n");
        std::fs::write(&page, text).expect("write page");

        let argv = CString::new("[\"build\"]").expect("argv");
        let cwd = CString::new(tmp.to_string_lossy().to_string()).expect("cwd");
        let ptr = unsafe { super::hvolt_run(argv.as_ptr(), cwd.as_ptr()) };
        let text = unsafe { CStr::from_ptr(ptr) }.to_string_lossy().to_string();
        unsafe { super::hvolt_free(ptr) };
        let value: serde_json::Value = serde_json::from_str(&text).expect("json");
        assert_ne!(value["code"], 0, "broken source must fail: {text}");
        let diags = value["diagnostics"].as_array().expect("diagnostics");
        assert!(!diags.is_empty(), "no diagnostics in {text}");
        let first = &diags[0];
        assert!(
            first["line"].as_u64().unwrap_or(0) > 0,
            "line missing: {first}"
        );
        assert_eq!(first["severity"], "error");
    }

    fn copy_tree(from: &std::path::Path, to: &std::path::Path) -> std::io::Result<()> {
        for entry in std::fs::read_dir(from)? {
            let entry = entry?;
            let target = to.join(entry.file_name());
            if entry.path().is_dir() {
                std::fs::create_dir_all(&target)?;
                copy_tree(&entry.path(), &target)?;
            } else {
                std::fs::copy(entry.path(), &target)?;
            }
        }
        Ok(())
    }
}
