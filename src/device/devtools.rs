//! Device interaction: logs, file transfer, uninstall.
//! Every operation resolves the install target the same way push/run do:
//! explicit device arg > persisted device_target > (remote host or local hdc).

use crate::device::hdc_proto::{self, HostIdentity};
use std::path::Path;
use std::time::Duration;

/// Resolve the target address the native transport understands: a USB serial
/// or `<host>:<port>` for a networked device.
fn target_addr(device: Option<&str>) -> Result<String, String> {
    if let Some(d) = device {
        if !d.trim().is_empty() {
            return Ok(d.trim().to_string());
        }
    }
    let cfg = crate::sdk::manage::load_global_config();
    if !cfg.device_target.is_empty() {
        return Ok(cfg.device_target);
    }
    Err("no device: pass one, or run `hvolt device connect <serial|ip:port>`".to_string())
}

fn identity() -> Result<HostIdentity, String> {
    HostIdentity::load_from_home()
}

/// Run one command on the device over the native hdc protocol and return its
/// output. No hdc binary is spawned.
pub(crate) fn sh(device: Option<&str>, cmdline: &str) -> Result<(String, String), String> {
    let addr = target_addr(device)?;
    let id = identity()?;
    let out = hdc_proto::shell(&addr, "Ver: 3.2.0", &id, cmdline, Duration::from_secs(90))?;
    Ok((addr, out))
}

// ─── log ───────────────────────────────────────────────────────────

#[derive(Default)]
pub struct LogOpts {
    pub level: Option<String>,  // D|I|W|E|F (hilog -L, comma list ok)
    pub tag: Option<String>,    // hilog -T, comma list up to 10
    pub domain: Option<String>, // hilog -D, comma list up to 5
    pub bundle: Option<String>, // client-side column match, + device pid filter when running
    pub keyword: Vec<String>,   // first goes to hilog -e regex, rest ANDed client-side
    pub follow: bool,
    pub crash: bool,
    pub tail: Option<usize>,
}

/// Build the on-device hilog argv (pure, unit-tested). Native hilog filters
/// are richer than the IDE surface; they are mapped 1:1, plus only what must be
/// client-side (bundle) stays client-side.
pub(crate) fn hilog_args(o: &LogOpts, pid: Option<&str>) -> Vec<String> {
    let mut a = vec!["hilog".to_string()];
    if o.crash {
        a.extend(["-L".into(), "E".into(), "-e".into(), "FAULT".into()]);
        return a;
    }
    if !o.follow {
        a.push("-x".into());
    }
    if let Some(l) = &o.level {
        a.extend(["-L".into(), l.clone()]);
    }
    if let Some(d) = &o.domain {
        a.extend(["-D".into(), d.clone()]);
    }
    if let Some(t) = &o.tag {
        a.extend(["-T".into(), t.clone()]);
    }
    if let Some(p) = pid {
        a.extend(["-P".into(), p.to_string()]); // cheap device-side narrowing
    }
    if let Some(k) = o.keyword.first() {
        a.extend(["-e".into(), format!("({})", k)]);
    }
    a
}

/// Client-side AND terms: keywords beyond the first (device-side -e handles #1).
pub(crate) fn client_terms(o: &LogOpts) -> Vec<String> {
    o.keyword.iter().skip(1).cloned().collect()
}

/// hilog line format: `MM-DD HH:MM:SS.mmm PID TID L <domain>/<proc>/<tag>: body`
/// — proc is the LAST bundle segment (demo, not com.example.demo) and scene
/// processes quote other bundles in bodies, so bundle filtering MUST be
/// column-anchored, never substring. None when the shape is unrecognized.
pub(crate) fn line_process(line: &str) -> Option<&str> {
    let colon = line.find(": ")?;
    let head = &line[..colon];
    let seg = head.rfind('/')?; // last of two-slash segment <domain>/<proc>/<tag>
    let proc_at = head[..seg].rfind('/').map(|i| i + 1).unwrap_or(seg);
    // sanity: token has no whitespace and the line started with a digit (timestamp)
    let p = &head[proc_at..seg];
    if p.is_empty() || p.contains(' ') {
        None
    } else {
        Some(p)
    }
}

pub(crate) fn line_matches(line: &str, terms: &[String]) -> bool {
    // keywords are case-insensitive (hilog renders tags uppercase; users type lowercase)
    let lc = line.to_lowercase();
    terms.iter().all(|t| lc.contains(&t.to_lowercase()))
}

pub(crate) fn bundle_matches_line(line: &str, bundle: &str) -> bool {
    match line_process(line) {
        Some(p) => bundle == p || bundle.ends_with(&format!(".{}", p)),
        None => false,
    }
}

pub fn log(o: &LogOpts, device: Option<&str>) -> Result<(), String> {
    // device-side pid filter whenever the bundle is running (follow or dump);
    // falls back to process-column matching when it is not
    let pid = o
        .bundle
        .as_deref()
        .and_then(|b| resolve_pid(b, device).ok());
    let mut args = hilog_args(o, pid.as_deref());
    // The native shell call is one round trip, so always ask hilog for the
    // dump form (`-x`); a streaming mode is rebuilt by polling and de-duping.
    if !args.iter().any(|a| a == "-x") {
        args.push("-x".to_string());
    }
    let cmdline = args.join(" ");
    let terms = client_terms(o);

    let keep = |lines: &str, sink: &mut Vec<String>| {
        for l in lines.lines() {
            if let Some(b) = &o.bundle {
                if !bundle_matches_line(l, b) && pid.is_none() {
                    continue;
                }
            }
            if line_matches(l, &terms) {
                sink.push(l.to_string());
            }
        }
    };

    if o.follow {
        let mut seen: std::collections::HashSet<String> = Default::default();
        loop {
            let (_, out) = sh(device, &cmdline)?;
            let mut kept = Vec::new();
            keep(&out, &mut kept);
            for l in kept {
                if seen.insert(l.clone()) {
                    println!("{}", l);
                }
            }
            std::thread::sleep(Duration::from_secs(2));
        }
    }

    let (_, stdout) = sh(device, &cmdline)?;
    let mut kept: Vec<String> = Vec::new();
    keep(&stdout, &mut kept);
    // "last N matching lines": filter first, THEN tail
    if let Some(n) = o.tail {
        if kept.len() > n {
            kept.drain(0..kept.len() - n);
        }
    }
    for l in &kept {
        println!("{}", l);
    }
    Ok(())
}

/// hdc path to use ON the device host: registered path wins; build-host path
/// is a last resort; callers report which path was used.
pub(crate) fn resolve_pid(bundle: &str, device: Option<&str>) -> Result<String, String> {
    let (_t, out) = sh(device, &format!("pidof {bundle}"))?;
    let pid = out.split_whitespace().next().unwrap_or("").to_string();
    if pid.is_empty() || !pid.chars().all(|c| c.is_ascii_digit()) {
        return Err(format!("{bundle} not running"));
    }
    Ok(pid)
}

// ─── file transfer ─────────────────────────────────────────────────

pub fn file_send(local: &Path, remote: &str, device: Option<&str>) -> Result<(), String> {
    if !local.exists() {
        return Err(format!("local file not found: {}", local.display()));
    }
    let abs = local
        .canonicalize()
        .map_err(|e| format!("resolve {}: {}", local.display(), e))?;
    let addr = target_addr(device)?;
    let id = identity()?;
    crate::device::hdc_proto::send_file(
        &addr,
        "Ver: 3.2.0",
        &id,
        &abs,
        remote,
        Duration::from_secs(120),
    )?;
    println!("✓ sent {} → {}:{}", local.display(), addr, remote);
    Ok(())
}

pub fn file_recv(remote: &str, local: &Path, device: Option<&str>) -> Result<(), String> {
    let addr = target_addr(device)?;
    let id = identity()?;
    let n = crate::device::hdc_proto::recv_file(
        &addr,
        "Ver: 3.2.0",
        &id,
        remote,
        local,
        Duration::from_secs(180),
    )?;
    println!(
        "✓ recv {}:{} → {} ({} bytes)",
        addr,
        remote,
        local.display(),
        n
    );
    Ok(())
}

// ─── ui capture (screenshot + layout dump) ─────────────────────────

fn timestamped(prefix: &str, ext: &str) -> std::path::PathBuf {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    std::path::PathBuf::from(format!("{}-{}.{}", prefix, secs, ext))
}

/// Device screenshot. snapshot_display FORCES .jpeg suffix on the device
/// side (measured: .png is rejected) — honor that, name locally as asked.
pub fn screenshot(device: Option<&str>, out: Option<&Path>) -> Result<(), String> {
    let remote = "/data/local/tmp/hvolt_capture.jpeg";
    // snapshot_display FORCES .jpeg on the device (measured: .png rejected)
    let (_t, _so) = sh(device, &format!("snapshot_display -f {remote}"))?;
    let staging = std::env::temp_dir().join("hvolt_capture.jpeg");
    let addr = target_addr(device)?;
    let id = identity()?;
    let n = crate::device::hdc_proto::recv_file(
        &addr,
        "Ver: 3.2.0",
        &id,
        remote,
        &staging,
        Duration::from_secs(180),
    )?;
    if n == 0 {
        return Err("screenshot recv returned 0 bytes".to_string());
    }
    let final_path = out
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| timestamped("screenshot", "jpeg"));
    std::fs::rename(&staging, &final_path).map_err(|e| format!("save: {}", e))?;
    println!("✓ {}", final_path.display());
    println!("  {n} bytes");
    Ok(())
}

/// Current ArkUI layout tree (uitest dumpLayout) as JSON on the local disk.
pub fn ui_dump(device: Option<&str>, out: Option<&Path>) -> Result<(), String> {
    let remote = "/data/local/tmp/hvolt_layout.json";
    let (_t, so) = sh(device, &format!("uitest dumpLayout -p {remote}"))?;
    if !so.contains("DumpLayout saved to") {
        return Err(format!("dumpLayout failed: {}", so.trim()));
    }
    let staging = std::env::temp_dir().join("hvolt_layout.json");
    let addr = target_addr(device)?;
    let id = identity()?;
    let n = crate::device::hdc_proto::recv_file(
        &addr,
        "Ver: 3.2.0",
        &id,
        remote,
        &staging,
        Duration::from_secs(180),
    )?;
    if n == 0 {
        return Err("layout recv returned 0 bytes".to_string());
    }
    let final_path = out
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| timestamped("layout", "json"));
    std::fs::rename(&staging, &final_path).map_err(|e| format!("save: {}", e))?;
    println!("✓ {}", final_path.display());
    println!("  {n} bytes");
    Ok(())
}

// ─── uninstall ─────────────────────────────────────────────────────

pub fn uninstall(bundle: &str, device: Option<&str>) -> Result<(), String> {
    let (t, out) = sh(device, &format!("bm uninstall -n {bundle}"))?;
    let text = out.trim();
    if text.contains("successfully") || text.contains("[Success]") || text.is_empty() {
        println!("✓ uninstalled {} from {}", bundle, t);
        Ok(())
    } else {
        Err(format!("uninstall failed: {text}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hilog_args_dump_default() {
        let o = LogOpts::default();
        assert_eq!(hilog_args(&o, None), vec!["hilog", "-x"]);
    }

    #[test]
    fn hilog_args_stream_filters_pid() {
        let o = LogOpts {
            follow: true,
            level: Some("E".into()),
            tag: Some("WebConsole".into()),
            ..Default::default()
        };
        let a = hilog_args(&o, Some("31415"));
        assert_eq!(a[0], "hilog");
        assert!(!a.contains(&"-x".to_string()));
        assert!(a.windows(2).any(|w| w == ["-L", "E"]));
        assert!(a.windows(2).any(|w| w == ["-T", "WebConsole"]));
        assert!(a.windows(2).any(|w| w == ["-P", "31415"]));
    }

    #[test]
    fn hilog_crash_shape() {
        let o = LogOpts {
            crash: true,
            ..Default::default()
        };
        assert_eq!(
            hilog_args(&o, None),
            vec!["hilog", "-L", "E", "-e", "FAULT"]
        );
    }

    #[test]
    fn client_terms_and_semantics() {
        let o = LogOpts {
            keyword: vec!["panic".into(), "napi".into()],
            ..Default::default()
        };
        assert_eq!(client_terms(&o), vec!["napi"]); // first goes device-side via -e
        assert!(line_matches("A NAPI panic", &client_terms(&o)));
        assert!(line_matches("lower napi here", &client_terms(&o))); // case-insensitive
        assert!(!line_matches("nothing relevant", &client_terms(&o)));
    }

    #[test]
    fn client_terms_empty_means_passthrough() {
        let o = LogOpts::default();
        assert!(client_terms(&o).is_empty());
        assert!(line_matches("anything", &[]));
    }

    #[test]
    fn bundle_matches_process_column_not_body() {
        let own = "10-10 09:17:51.607  1754 62558 I C04200/demo/WebConsole: hello from app";
        let noise = "10-10 09:17:51.685  6348 6348 I A01B01/com.ohos.sceneboard/HOME: appIndex:0,uninstallable:true; 2_1_0,com.example.demo,typeid:0";
        assert!(bundle_matches_line(own, "com.example.demo"));
        assert!(
            !bundle_matches_line(noise, "com.example.demo"),
            "body mention must NOT match"
        );
        // exact proc name and last-segment suffix both work
        assert!(bundle_matches_line(own, "demo"));
        assert!(!bundle_matches_line("not a hilog line", "demo"));
    }
}
