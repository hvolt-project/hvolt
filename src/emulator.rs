//! emulator: drive the official HarmonyOS Emulator CLI.
//!
//! Command surface measured from `Emulator -help` (26.0.0.200, mac arm64):
//! -create/-delete/-imageList/-install/-uninstall/-list/-start/-stop,
//! plus -instance scenario controls (shake/battery/gps/screenshot).
//! hvolt passes through what matters for headless workflows; the GUI stays
//! the IDE's job.

use crate::platform;
use std::path::PathBuf;
use std::process::Command;

/// Locate the Emulator binary: DevEco installs (mac/win), then PATH.
pub fn find_emulator_bin(env: Option<&crate::sdk::SdkEnv>) -> Option<PathBuf> {
    if let Some(e) = env {
        if let Some(ref emu) = e.emulator {
            if emu.exists() {
                return Some(emu.clone());
            }
        }
    }
    let cands: Vec<PathBuf> = if platform::is_windows() {
        [
            "C:\\Program Files\\Huawei\\DevEco Studio\\tools\\emulator\\Emulator.exe",
            "C:\\Program Files\\Huawei\\DevEco-Studio\\tools\\emulator\\Emulator.exe",
        ]
        .iter()
        .map(PathBuf::from)
        .collect()
    } else if cfg!(target_os = "macos") {
        vec![
            PathBuf::from("/Applications/DevEco-Studio.app/Contents/tools/emulator/Emulator"),
            PathBuf::from("/Applications/DevEco Studio.app/Contents/tools/emulator/Emulator"),
        ]
    } else {
        vec![] // Linux: no emulator binary ships with DevEco there
    };
    cands
        .into_iter()
        .find(|p| p.exists())
        .or_else(|| platform::on_path("Emulator").or_else(|| platform::on_path("emulator")))
}

/// The typed command set (mirrors the verified CLI surface).
pub enum EmuCmd {
    List {
        details: bool,
    },
    Start {
        name: String,
        no_window: bool,
    },
    Stop {
        name: String,
    },
    Create {
        name: String,
        device_type: String,
        os_version: Option<String>,
        memory: Option<u32>,
    },
    Delete {
        name: String,
        force: bool,
    },
    Images {
        downloaded_only: bool,
    },
    ImageInstall {
        device_type: String,
        os_version: String,
        force: bool,
    },
    AcceptLicense,
}

/// Assemble the argv for an emulator action (pure; unit-tested).
/// Returns args AFTER the binary path.
pub(crate) fn emu_args(action: &EmuCmd) -> Vec<String> {
    match action {
        EmuCmd::List { details } => {
            let mut a = vec!["-list".into()];
            if *details {
                a.push("-details".into());
            }
            a
        }
        EmuCmd::Start { name, no_window } => {
            let mut a = vec!["-start".into(), name.clone()];
            if *no_window {
                a.push("-noWindow".into());
            }
            a
        }
        EmuCmd::Stop { name } => vec!["-stop".into(), name.clone()],
        EmuCmd::Create {
            name,
            device_type,
            os_version,
            memory,
        } => {
            let mut a = vec![
                "-create".into(),
                name.clone(),
                "-deviceType".into(),
                device_type.clone(),
            ];
            if let Some(v) = os_version {
                a.extend(vec!["-osVersion".into(), v.clone()]);
            }
            if let Some(m) = memory {
                a.extend(vec!["-memory".into(), m.to_string()]);
            }
            a
        }
        EmuCmd::Delete { name, force } => {
            let mut a = vec!["-delete".into(), name.clone()];
            if *force {
                a.push("-force".into());
            }
            a
        }
        EmuCmd::Images { downloaded_only } => {
            let mut a = vec!["-imageList".into()];
            if *downloaded_only {
                a.extend(vec!["-downloaded".into(), "true".into()]);
            }
            a
        }
        EmuCmd::ImageInstall {
            device_type,
            os_version,
            force,
        } => {
            let mut a = vec![
                "-install".into(),
                "-deviceType".into(),
                device_type.clone(),
                "-osVersion".into(),
                os_version.clone(),
            ];
            if *force {
                a.push("-force".into());
            }
            a
        }
        EmuCmd::AcceptLicense => vec!["-license".into(), "accept".into()],
    }
}

/// Official failure collocations observed from the real CLI (create/
/// delete/start/stop/install + "cannot be empty"). License prose mentions
/// "fail" in legal sentences — collocation keeps it out of the net.
pub(crate) fn official_failure(text: &str) -> bool {
    let t = text.to_lowercase();
    [
        "create fail",
        "delete fail",
        "start fail",
        "stop fail",
        "install fail",
        "uninstall fail",
        "cannot be empty",
        "image is not found",
        "no such image",
    ]
    .iter()
    .any(|p| t.contains(p))
}

pub fn run(action: &EmuCmd) -> Result<(), String> {
    let bin = find_emulator_bin(None)
        .ok_or("Emulator not found — it ships with DevEco Studio (Windows/macOS); Linux has no official emulator binary")?;
    let args = emu_args(action);
    let out = Command::new(&bin)
        .args(&args)
        .output()
        .map_err(|e| format!("emulator: {}", e))?;
    let so = String::from_utf8_lossy(&out.stdout);
    let se = String::from_utf8_lossy(&out.stderr);
    // images list: the official CLI answers raw JSON — render a table and
    // show the exact quoted download command
    if matches!(action, EmuCmd::List { details: true }) {
        if let Some(table) = render_instances(&so) {
            print!("{}", table);
            return if out.status.success() {
                Ok(())
            } else {
                Err("emulator exit".into())
            };
        }
    }
    if matches!(action, EmuCmd::Images { .. }) {
        if let Some(table) = render_images(&so) {
            print!("{}", table);
            return if out.status.success() {
                Ok(())
            } else {
                Err("emulator exit".into())
            };
        }
    }
    print!("{}{}", so, se);
    // The official CLI returns 0 even on "Device create fail." (measured), but
    // a blanket "fail" substring is wrong: the license agreement text contains
    // "failure to fulfill obligations" and accepting it succeeds. Match only the
    // verb+fail collocations the tool actually emits.
    let mutating = !matches!(action, EmuCmd::List { .. } | EmuCmd::Images { .. });
    if mutating && official_failure(&format!("{}{}", so, se)) {
        return Err(
            "emulator reported failure (official exit code is unreliable — see output above)"
                .into(),
        );
    }
    if !out.status.success() {
        if so.contains("agreement") || se.contains("agreement") {
            println!("→ first run needs the agreement: hvolt emulator license");
        }
        return Err(format!("emulator exit {}", out.status.code().unwrap_or(-1)));
    }
    Ok(())
}

/// -list -details answers a wide JSON per instance — render the columns a
/// an operator actually scans.
pub(crate) fn render_instances(raw: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(raw.trim()).ok()?;
    let arr = v.as_array()?;
    // column widths adapt to content (a 27-char osVersion
    // overflowed the fixed {:<22} and desynced the table)
    let rows: Vec<(String, String, String, String, String, String)> = arr
        .iter()
        .map(|it| {
            (
                it["name"].as_str().unwrap_or("?").to_string(),
                it["deviceType"].as_str().unwrap_or("?").to_string(),
                it["os.osVersion"].as_str().unwrap_or("?").to_string(),
                it["hw.ramSize"].as_str().unwrap_or("?").to_string(),
                it["hw.cpu.ncore"].as_str().unwrap_or("?").to_string(),
                if it["isRunning"].as_bool().unwrap_or(false) {
                    "● running".into()
                } else {
                    "  stopped".into()
                },
            )
        })
        .collect();
    let wn = rows.iter().map(|r| r.0.len()).max().unwrap_or(4).max(4) + 2;
    let wt = rows.iter().map(|r| r.1.len()).max().unwrap_or(4).max(4) + 2;
    let wo = rows.iter().map(|r| r.2.len()).max().unwrap_or(9).max(9) + 2;
    let mut out = String::from("Instances:\n");
    out.push_str(&format!(
        "  {:<wn$} {:<wt$} {:<wo$} {:>4} {:>4}  {}\n",
        "name", "type", "osVersion", "ram", "cpu", "state"
    ));
    for r in &rows {
        out.push_str(&format!(
            "  {:<wn$} {:<wt$} {:<wo$} {:>4} {:>4}  {}\n",
            r.0, r.1, r.2, r.3, r.4, r.5
        ));
    }
    Some(out)
}

/// Format the official -imageList JSON into "osVersion  deviceType  state"
/// rows + a copy-pasteable download example. None when the payload is not
/// the expected JSON (the raw payload is passed through unchanged).
pub(crate) fn render_images(raw: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(raw.trim()).ok()?;
    let arr = v.as_array()?;
    let mut rows = String::new();
    let mut first_dl: Option<String> = None;
    for it in arr {
        let osv = it["osVersion"].as_str()?;
        let dt = it["deviceType"].as_str().unwrap_or("?");
        let dl = it["downloaded"].as_str() == Some("true");
        let rel = it["releaseType"].as_str().unwrap_or("");
        rows.push_str(&format!(
            "  {:<22} {:<16} {}  {}\n",
            osv,
            dt,
            if dl { "✓ downloaded" } else { "  available" },
            rel
        ));
        if !dl && first_dl.is_none() {
            first_dl = Some(osv.to_string());
        }
    }
    let mut out = format!("System images:\n{}\n", rows);
    if let Some(v) = first_dl {
        out.push_str(&format!(
            "download: hvolt emulator images download \"{}\"\n",
            v
        ));
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn argv_create_full_and_minimal() {
        let a = emu_args(&EmuCmd::Create {
            name: "dev1".into(),
            device_type: "phone".into(),
            os_version: Some("HarmonyOS-26.0.0".into()),
            memory: Some(4096),
        });
        assert_eq!(
            a,
            vec![
                "-create",
                "dev1",
                "-deviceType",
                "phone",
                "-osVersion",
                "HarmonyOS-26.0.0",
                "-memory",
                "4096"
            ]
        );
        let a = emu_args(&EmuCmd::Create {
            name: "d".into(),
            device_type: "tablet".into(),
            os_version: None,
            memory: None,
        });
        assert_eq!(a, vec!["-create", "d", "-deviceType", "tablet"]);
    }

    #[test]
    fn argv_start_stop_delete() {
        assert_eq!(
            emu_args(&EmuCmd::Start {
                name: "e1".into(),
                no_window: true
            }),
            vec!["-start", "e1", "-noWindow"]
        );
        assert_eq!(
            emu_args(&EmuCmd::Start {
                name: "e1".into(),
                no_window: false
            }),
            vec!["-start", "e1"]
        );
        assert_eq!(
            emu_args(&EmuCmd::Stop { name: "e1".into() }),
            vec!["-stop", "e1"]
        );
        assert_eq!(
            emu_args(&EmuCmd::Delete {
                name: "e1".into(),
                force: true
            }),
            vec!["-delete", "e1", "-force"]
        );
    }

    #[test]
    fn argv_images_and_license() {
        assert_eq!(
            emu_args(&EmuCmd::Images {
                downloaded_only: true
            }),
            vec!["-imageList", "-downloaded", "true"]
        );
        assert_eq!(
            emu_args(&EmuCmd::ImageInstall {
                device_type: "phone".into(),
                os_version: "v".into(),
                force: false
            }),
            vec!["-install", "-deviceType", "phone", "-osVersion", "v"]
        );
        assert_eq!(emu_args(&EmuCmd::AcceptLicense), vec!["-license", "accept"]);
    }

    #[test]
    fn failure_collocation_no_license_false_positive() {
        assert!(official_failure("Device create fail."));
        assert!(official_failure(
            "The virtual device version cannot be empty."
        ));
        // license prose (measured 9 "fail" occurrences) must not trip it
        assert!(!official_failure(
            "failure to fulfill obligations may cause negotiation fails"
        ));
        assert!(!official_failure(
            "Failed to create config directory is mentioned in section 4"
        ));
        // ^ a standalone "failed" without the verb collocation is not matched
    }

    #[test]
    fn instances_details_table_renders_or_falls_back() {
        let raw = r#"[{"name":"Pura 90","deviceType":"phone","os.osVersion":"HarmonyOS 6.1.1(24)","hw.ramSize":"8192","hw.cpu.ncore":"4","isRunning":true}]"#;
        let t = render_instances(raw).unwrap();
        assert!(
            t.contains("Pura 90") && t.contains("● running") && t.contains("6.1.1"),
            "{}",
            t
        );
        assert!(render_instances("plain text list").is_none());
    }

    #[test]
    fn images_table_renders_and_flags_quotes() {
        let raw = r#"[{"osVersion":"HarmonyOS 6.1.1(24)","deviceType":"2in1","downloaded":"false","releaseType":"Release"},{"osVersion":"HarmonyOS 5.0.0(12)","deviceType":"phone","downloaded":"true","releaseType":"Release"}]"#;
        let t = render_images(raw).unwrap();
        assert!(t.contains("HarmonyOS 6.1.1(24)"), "{}", t);
        assert!(t.contains("✓ downloaded"));
        assert!(t.contains("available"));
        // the suggested command MUST quote the spaced version string
        assert!(t.contains(r#"download "HarmonyOS 6.1.1(24)""#), "{}", t);
        // non-JSON stays None (raw passthrough)
        assert!(render_images("not json").is_none());
    }

    #[test]
    fn discovery_returns_existing_paths_only() {
        // with DevEco installed here it must find the binary; without it,
        // None — never a fabricated path
        if let Some(p) = find_emulator_bin(None) {
            assert!(p.exists(), "discovery must only return existing files");
        }
    }
}
