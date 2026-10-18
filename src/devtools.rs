//! Device interaction: logs, file transfer, uninstall.
//! Every operation resolves the install target the same way push/run do:
//! explicit device arg > persisted device_target > (remote host or local hdc).

use crate::sdk;
use crate::target;
use std::path::Path;
use std::process::{Command, Stdio};

/// Run `hdc <args...>` against the resolved device, locally or on the
/// registered device host. Returns (target, stdout).
pub(crate) fn hdc_exec<A: AsRef<std::ffi::OsStr>>(
    device: Option<&str>,
    args: &[A],
) -> Result<(String, String), String> {
    let (serial, local_hdc) = target::resolve(device)?;
    let cfg = sdk::load_global_config();
    if cfg.device_host.is_empty() {
        let mut c = Command::new(&local_hdc);
        c.arg("-t").arg(&serial).args(args);
        let out = c.output().map_err(|e| format!("hdc: {}", e))?;
        Ok((serial, String::from_utf8_lossy(&out.stdout).to_string()))
    } else {
        // the hdc binary that exists is the one on the DEVICE HOST — the
        // build host's path means nothing over there (regression test)
        let hdc_str = remote_hdc_path(&cfg.device_hdc, &local_hdc);
        // full argv including -t serial, quoted for the device host shell
        let mut parts = vec![
            target::quote_arg(&hdc_str, target::DeviceShell::Posix),
            "-t".into(),
            target::quote_arg(&serial, target::DeviceShell::Posix),
        ];
        parts.extend(args.iter().map(|a| {
            target::quote_arg(
                a.as_ref().to_string_lossy().as_ref(),
                target::DeviceShell::Posix,
            )
        }));
        debug_assert!(!parts.is_empty());
        let remote = parts.join(" ");
        let out = Command::new("ssh")
            .args(target::ssh_args(&cfg.device_host, &remote))
            .output()
            .map_err(|e| format!("ssh: {}", e))?;
        Ok((serial, String::from_utf8_lossy(&out.stdout).to_string()))
    }
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
    let args = hilog_args(o, pid.as_deref());
    let cfg = sdk::load_global_config();
    let (serial, hdc) = target::resolve(device)?;
    let terms = client_terms(o);

    if o.follow {
        // streaming: line-by-line, no buffering
        let mut cmd = if cfg.device_host.is_empty() {
            let mut c = Command::new(&hdc);
            c.arg("-t").arg(&serial).arg("shell").args(&args);
            c
        } else {
            let mut parts = vec![
                target::quote_arg(
                    &remote_hdc_path(&cfg.device_hdc, &hdc).to_string(),
                    target::DeviceShell::Posix,
                ),
                "-t".into(),
                target::quote_arg(&serial, target::DeviceShell::Posix),
                "shell".into(),
            ];
            parts.extend(
                args.iter()
                    .map(|a| target::quote_arg(a, target::DeviceShell::Posix)),
            );
            let mut c = Command::new("ssh");
            c.args(target::ssh_args(&cfg.device_host, &parts.join(" ")));
            c
        };
        cmd.stdout(Stdio::piped());
        let mut child = cmd.spawn().map_err(|e| format!("spawn: {}", e))?;
        let rd = std::io::BufReader::new(child.stdout.take().unwrap());
        use std::io::BufRead;
        let bundle = o.bundle.as_deref();
        for line in rd.lines().map_while(|l| l.ok()) {
            if let Some(b) = bundle {
                if !bundle_matches_line(&line, b) && pid.is_none() {
                    continue;
                }
            }
            if !line_matches(&line, &terms) {
                continue;
            }
            println!("{}", line);
        }
        return Ok(());
    }

    let mut full = vec!["shell".to_string()];
    full.extend(args);
    let (_t, stdout) = hdc_exec(device, &full)?;
    let mut kept: Vec<&str> = Vec::new();
    for l in stdout.lines() {
        if let Some(b) = &o.bundle {
            if !bundle_matches_line(l, b) && pid.is_none() {
                continue;
            }
        }
        if !line_matches(l, &terms) {
            continue;
        }
        kept.push(l);
    }
    // "last N matching lines": filter first, THEN tail — the previous order
    // (tail raw lines then filter) could return fewer than N
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
pub(crate) fn remote_hdc_path(registered: &str, local_hdc: &Path) -> String {
    if !registered.is_empty() {
        return registered.to_string();
    }
    local_hdc.to_string_lossy().into_owned()
}

fn resolve_pid(bundle: &str, device: Option<&str>) -> Result<String, String> {
    let (_t, out) = hdc_exec(device, &["shell", "pidof", bundle])?;
    let pid = out.split_whitespace().next().unwrap_or("").to_string();
    if pid.is_empty() || !pid.chars().all(|c| c.is_ascii_digit()) {
        return Err(format!("{} not running", bundle));
    }
    Ok(pid)
}

// ─── file transfer ─────────────────────────────────────────────────

pub fn file_send(local: &Path, remote: &str, device: Option<&str>) -> Result<(), String> {
    if !local.exists() {
        return Err(format!("local file not found: {}", local.display()));
    }
    // ABSOLUTE path: in cross-machine mode the send executes on the device
    // host whose cwd is unrelated to this project (relative paths broke there)
    let abs = local
        .canonicalize()
        .map_err(|e| format!("resolve {}: {}", local.display(), e))?;
    let (t, out) = hdc_exec(device, &["file", "send", &abs.to_string_lossy(), remote])?;
    if out.contains("FileTransfer finish") {
        println!("✓ sent {} → {}:{}", local.display(), t, remote);
        Ok(())
    } else {
        Err(format!("file send failed: {}", out.trim()))
    }
}

pub fn file_recv(remote: &str, local: &Path, device: Option<&str>) -> Result<(), String> {
    let (t, out) = hdc_exec(device, &["file", "recv", remote, &local.to_string_lossy()])?;
    if out.contains("FileTransfer finish") {
        println!("✓ recv {}:{} → {}", t, remote, local.display());
        Ok(())
    } else {
        Err(format!("file recv failed: {}", out.trim()))
    }
}

// ─── ui capture (screenshot + layout dump) ─────────────────────────

/// Bring a file that lives on the DEVICE HOST (cross-machine mode) back to
/// the invoking machine. No-op in local mode (`already` is the final path).
fn pull_from_device_host(already: &Path, remote_on_host: &str) -> Result<(), String> {
    let cfg = sdk::load_global_config();
    if cfg.device_host.is_empty() {
        return Ok(()); // hdc recv landed locally already
    }
    let out = std::process::Command::new("scp")
        .args(target::scp_pull_args(
            &cfg.device_host,
            remote_on_host,
            already,
        ))
        .output()
        .map_err(|e| format!("scp pull: {}", e))?;
    if !out.status.success() {
        return Err(format!(
            "pull from device host failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(())
}

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
    // snapshot_display stdout dialect is unreliable; success is proven by the recv below
    let (_t, _so) = hdc_exec(device, &["shell", "snapshot_display", "-f", remote])?;
    let staging = std::env::temp_dir().join("hvolt_capture.jpeg");
    let (_t, ro) = hdc_exec(
        device,
        &["file", "recv", remote, &staging.to_string_lossy()],
    )?;
    if !ro.contains("FileTransfer finish") {
        let _ = std::fs::remove_file(&staging);
        let _ = hdc_exec(device, &["shell", "rm", remote]);
        return Err(format!("screenshot recv failed: {}", ro.trim()));
    }
    pull_from_device_host(&staging, &staging.to_string_lossy())?;
    let final_path = out
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| timestamped("screenshot", "jpeg"));
    std::fs::rename(&staging, &final_path).map_err(|e| format!("save: {}", e))?;
    let _ = hdc_exec(device, &["shell", "rm", remote]);
    let size = std::fs::metadata(&final_path).map(|m| m.len()).unwrap_or(0);
    println!("✓ {}", final_path.display());
    println!("  {} bytes", size);
    Ok(())
}

/// Current ArkUI layout tree (uitest dumpLayout) as JSON on the local disk.
pub fn ui_dump(device: Option<&str>, out: Option<&Path>) -> Result<(), String> {
    let remote = "/data/local/tmp/hvolt_layout.json";
    let (_t, so) = hdc_exec(device, &["shell", "uitest", "dumpLayout", "-p", remote])?;
    if !so.contains("DumpLayout saved to") {
        return Err(format!("dumpLayout failed: {}", so.trim()));
    }
    let staging = std::env::temp_dir().join("hvolt_layout.json");
    let (_t, ro) = hdc_exec(
        device,
        &["file", "recv", remote, &staging.to_string_lossy()],
    )?;
    if !ro.contains("FileTransfer finish") {
        let _ = std::fs::remove_file(&staging);
        let _ = hdc_exec(device, &["shell", "rm", remote]);
        return Err(format!("layout recv failed: {}", ro.trim()));
    }
    pull_from_device_host(&staging, &staging.to_string_lossy())?;
    let final_path = out
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| timestamped("layout", "json"));
    std::fs::rename(&staging, &final_path).map_err(|e| format!("save: {}", e))?;
    let _ = hdc_exec(device, &["shell", "rm", remote]);
    let size = std::fs::metadata(&final_path).map(|m| m.len()).unwrap_or(0);
    println!("✓ {}", final_path.display());
    println!("  {} bytes", size);
    Ok(())
}

// ─── uninstall ─────────────────────────────────────────────────────

pub fn uninstall(bundle: &str, device: Option<&str>) -> Result<(), String> {
    let (t, out) = hdc_exec(device, &["uninstall", bundle])?;
    if out.contains("[Success]") || out.contains("uninstall bundle successfully") {
        println!("✓ uninstalled {} from {}", bundle, t);
        Ok(())
    } else {
        Err(format!("uninstall failed: {}", out.trim()))
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
    fn remote_uses_registered_device_host_hdc_not_local_path() {
        // regression for the 127-file-not-found cross-machine bug
        let registered =
            "/Applications/DevEco-Studio.app/Contents/sdk/default/openharmony/toolchains/hdc";
        let local =
            std::path::Path::new("/home/builder/.hvolt/sdk/default/openharmony/toolchains/hdc");
        assert_eq!(remote_hdc_path(registered, local), registered);
        let empty = remote_hdc_path("", local);
        assert_eq!(empty, local.to_string_lossy());
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
