// hdc: HarmonyOS Device Connector wrapper.
//
// Provides find_hdc (path resolution) and hdc_command (tconn-aware Command builder).

use std::path::PathBuf;
use std::process::Command;

/// Locate the hdc binary from PATH.
pub fn find_hdc() -> Option<PathBuf> {
    crate::platform::on_path("hdc")
}

/// Build a Command pre-configured with tconn (if TCP address) and -t.
/// Returns (hdc_path, Command). Caller adds remaining args and executes.
#[cfg_attr(not(feature = "agc-local"), allow(dead_code))] // used by the optional AGC backend
pub fn hdc_command(device: Option<&str>) -> Result<(PathBuf, Command), String> {
    let hdc = find_hdc().ok_or(
        "hdc not found on PATH.\n\
        Install HarmonyOS SDK (hdc is in the toolchains component),\n\
        or place hdc on your PATH.",
    )?;
    let mut cmd = Command::new(&hdc);

    if let Some(d) = device {
        if d.contains(':') {
            // TCP device: connect first with timeout, then target
            let mut conn = Command::new(&hdc);
            conn.arg("tconn").arg(d);
            // output() (not status()) so stdout is captured. No timeout here:
            // a wedged hdc blocks this call indefinitely. A watchdog is a
            // separate work item and is not implemented.
            let conn_out = conn.output().map_err(|e| format!("hdc tconn: {}", e))?;
            let conn_stdout = String::from_utf8_lossy(&conn_out.stdout);
            if !conn_out.status.success() {
                let err = conn_stdout.trim();
                return Err(format!("hdc tconn {} failed: {}", d, err));
            }
            cmd.arg("-t").arg(d);
        } else {
            cmd.arg("-t").arg(d);
        }
    }

    Ok((hdc, cmd))
}
