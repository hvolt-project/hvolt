// target.rs — persistent install target (device host + serial).
//
// The device is often NOT attached to the build machine (remote build server +
// USB device on the workstation). `hvolt device connect <ip:port>` records the
// target in ~/.hvolt/config.json so subsequent builds know where to install:
//
//   hvolt run                     # after the build, push+launch to the target
//
// Two transports:
//   - local: hdc on this machine reaches the device (USB or TCP via `hdc tconn`)
//   - remote: host field is "user@ip" — the signed HAP is scp'ed there and
//     `hdc install` runs on that machine (its own ~/.hvolt supplies hdc + udid)

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::sdk;

/// Resolve the install target. Priority: explicit CLI arg > config > any local
/// hdc-visible device. Returns (serial_or_addr, hdc_on_this_machine).
/// THE hdc discovery chain (PATH → configured/DevEco SDK roots). Every
/// command that talks to a device must resolve hdc through this single
/// function — device list previously used a narrower chain and claimed
/// "hdc not found" while screenshot/log worked fine on the same machine.
pub fn find_hdc_unified() -> Option<PathBuf> {
    crate::hdc::find_hdc()
        .or_else(|| sdk::sdk_root().and_then(|root| sdk::find_hdc_from_sdk(&root)))
}

pub fn resolve(cli_device: Option<&str>) -> Result<(String, PathBuf), String> {
    let hdc = find_hdc_unified()
        .ok_or("hdc not found (PATH, DevEco, or ~/.hvolt SDK) — run `hvolt sdk download <v>`")?;

    let serial = cli_device.map(|s| s.to_string()).or_else(|| {
        let cfg = sdk::load_global_config();
        if cfg.device_target.is_empty() {
            None
        } else {
            Some(cfg.device_target.clone())
        }
    });

    match serial {
        Some(s) => Ok((s, hdc)),
        None => {
            // fall back: exactly one local target?
            let out = Command::new(&hdc)
                .arg("list")
                .arg("targets")
                .output()
                .map_err(|e| format!("hdc list targets: {}", e))?;
            let lines: Vec<String> = String::from_utf8_lossy(&out.stdout)
                .lines()
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty() && l != "[Empty]")
                .collect();
            match lines.len() {
                0 => Err("no install target: connect a device or run `hvolt device connect <ip:port>`".into()),
                1 => Ok((lines.into_iter().next().unwrap(), hdc)),
                n => Err(format!("{} devices connected — pick one with `hvolt device connect <serial>` or pass the address", n)),
            }
        }
    }
}

/// Build a targeted hdc Command (tconn first for ip:port targets).
pub fn hdc_targeted_command(cli_device: Option<&str>) -> Result<(String, Command), String> {
    let (serial, hdc) = resolve(cli_device)?;
    let mut cmd = Command::new(&hdc);
    if serial.contains(':') {
        // make sure the TCP device is attached to the local hdc server
        let mut conn = Command::new(&hdc);
        conn.arg("tconn").arg(&serial);
        let out = conn.output().map_err(|e| format!("hdc tconn: {}", e))?;
        let stdout = String::from_utf8_lossy(&out.stdout);
        if !out.status.success() || stdout.contains("[Fail]") {
            return Err(format!("tconn {} failed: {}", serial, stdout.trim()));
        }
    }
    cmd.arg("-t").arg(&serial);
    Ok((serial, cmd))
}

/// Connect a TCP device and persist it as the install target.
pub fn connect_and_save(addr: &str) -> Result<(), String> {
    let (_, hdc) = resolve(Some(addr))?; // hdc path on this machine
    if addr.contains(':') {
        let mut conn = Command::new(&hdc);
        conn.arg("tconn").arg(addr);
        let out = conn.output().map_err(|e| format!("hdc tconn: {}", e))?;
        let stdout = String::from_utf8_lossy(&out.stdout);
        if !out.status.success() || stdout.contains("[Fail]") {
            return Err(format!("tconn {} failed: {}", addr, stdout.trim()));
        }
    }
    let mut cfg = sdk::load_global_config();
    cfg.device_target = addr.to_string();
    sdk::save_global_config(&cfg)?;
    println!(
        "✓ Install target set: {} (saved to ~/.hvolt/config.json)",
        addr
    );
    Ok(())
}

// ─── cross-machine install (build host ≠ device host) ─────────────

/// hvolt's dedicated device-channel key (generated on demand by `device remote`).
pub(crate) fn device_key_path() -> PathBuf {
    sdk::home_dir().join(".hvolt").join("device_ed25519")
}

/// argv for the identity option, given the resolved key path (None → no -i).
pub(crate) fn key_args_for(key: Option<&Path>) -> Vec<String> {
    match key {
        Some(k) => vec!["-i".into(), k.to_string_lossy().into_owned()],
        None => Vec::new(),
    }
}

/// Split "user@host[:port]" into (target, port-or-None).
pub(crate) fn split_host_port(spec: &str) -> (String, Option<u16>) {
    // only split on the LAST colon, and only if numeric (ipv6-safe enough for configs)
    match spec.rsplit_once(':') {
        Some((rest, p)) if p.parse::<u16>().is_ok() => (rest.to_string(), Some(p.parse().unwrap())),
        _ => (spec.to_string(), None),
    }
}

/// scp argv pushing `local` onto the device host under `remote_dir`.
pub(crate) fn scp_args(spec: &str, local: &Path, remote_dir: &str) -> Vec<String> {
    let k = scp_key();
    scp_args_keyed(k.as_deref(), spec, local, remote_dir)
}

fn scp_key() -> Option<PathBuf> {
    let k = device_key_path();
    k.exists().then_some(k)
}

pub(crate) fn scp_args_keyed(
    key: Option<&Path>,
    spec: &str,
    local: &Path,
    remote_dir: &str,
) -> Vec<String> {
    let (host, port) = split_host_port(spec);
    let mut a = key_args_for(key);
    a.extend(vec![
        "-o".into(),
        "BatchMode=yes".into(),
        "-o".into(),
        "ConnectTimeout=15".into(),
    ]);
    if let Some(p) = port {
        a.push("-P".into());
        a.push(p.to_string());
    }
    let name = local
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    a.push(local.to_string_lossy().into_owned());
    a.push(format!("{}:{}/{}", host, remote_dir, name));
    a
}

/// ssh argv running a single remote command (port-aware).
/// scp PULL args (device host → invoking machine), same key/port policy.
pub(crate) fn scp_pull_args(spec: &str, remote_path: &str, local: &Path) -> Vec<String> {
    let (host, port) = split_host_port(spec);
    let mut a = scp_key()
        .map(|k| vec!["-i".into(), k.to_string_lossy().into_owned()])
        .unwrap_or_default();
    a.extend(vec![
        "-o".into(),
        "BatchMode=yes".into(),
        "-o".into(),
        "ConnectTimeout=15".into(),
    ]);
    if let Some(p) = port {
        a.push("-P".into());
        a.push(p.to_string());
    }
    a.push(format!("{}:{}", host, remote_path));
    a.push(local.to_string_lossy().into_owned());
    a
}

pub(crate) fn ssh_args(spec: &str, remote_cmd: &str) -> Vec<String> {
    let k = ssh_key();
    ssh_args_keyed(k.as_deref(), spec, remote_cmd)
}

fn ssh_key() -> Option<PathBuf> {
    let k = device_key_path();
    k.exists().then_some(k)
}

pub(crate) fn ssh_args_keyed(key: Option<&Path>, spec: &str, remote_cmd: &str) -> Vec<String> {
    let (host, port) = split_host_port(spec);
    let mut a: Vec<String> = key_args_for(key);
    if let Some(p) = port {
        a.push("-p".into());
        a.push(p.to_string());
    }
    a.push("-o".into());
    a.push("BatchMode=yes".into());
    a.push("-o".into());
    a.push("ConnectTimeout=15".into());
    a.push(host);
    a.push(remote_cmd.to_string());
    a
}

/// Device-host shell dialect for remote command lines.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum DeviceShell {
    Posix,
    #[allow(dead_code)] // reserved dialect: reachable if a pwsh-default host registers
    PowerShell,
    Cmd,
}

/// Quote one parameter for the device host's shell dialect (pure, tested).
pub(crate) fn quote_arg(p: &str, shell: DeviceShell) -> String {
    match shell {
        DeviceShell::Posix => format!("'{}'", p.replace('\'', "'\\''")),
        DeviceShell::PowerShell => format!("'{}'", p.replace('\'', "''")),
        DeviceShell::Cmd => {
            // defense-in-depth: probe_host already refuses pure-cmd hosts,
            // but the quoting helper must not be the layer that leaks
            // & | < > ^
            if p.chars()
                .any(|c| matches!(c, ' ' | '&' | '|' | '<' | '>' | '^' | '"' | '%'))
            {
                format!("\"{}\"", p.replace('"', "\\\""))
            } else {
                p.to_string()
            }
        }
    }
}

pub(crate) fn shell_join(sub: &[&str], shell: DeviceShell) -> String {
    sub.iter()
        .map(|p| quote_arg(p, shell))
        .collect::<Vec<_>>()
        .join(" ")
}

/// hdc command line honoring the host shell dialect.
pub(crate) fn remote_hdc_cmd(hdc: &str, serial: &str, sub: &[&str], shell: DeviceShell) -> String {
    format!(
        "{} -t {} {}",
        quote_arg(hdc, shell),
        quote_arg(serial, shell),
        shell_join(sub, shell)
    )
}

/// Network targets ("ip:port") need `hdc tconn` on the device host before
/// any -t addressing works (the host's hdc server may have dropped it).
pub(crate) fn needs_tconn(serial: &str) -> bool {
    serial.contains(':')
}

/// Detect device-host shell + absolute staging dir in one ssh round trip.
/// Windows OpenSSH with a scoop bash as DefaultShell answers POSIX; plain
/// cmd answers with the literal "%HOME%..." unexpanded.
pub(crate) struct DeviceHost {
    pub shell: DeviceShell,
    pub staging: String, // absolute dir for the hap
}

pub(crate) fn probe_host(spec: &str) -> Result<DeviceHost, String> {
    // sh -c only exists where a POSIX shell does; cmd echoes the marker back.
    let probe = "sh -c 'echo HVSHELL_OK'; echo HVID:$HOME";
    let out = Command::new("ssh")
        .args(ssh_args(spec, probe))
        .output()
        .map_err(|e| format!("ssh probe: {}", e))?;
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    let shell = if text.contains("HVSHELL_OK") {
        DeviceShell::Posix
    } else {
        DeviceShell::Cmd
    };
    let id_line = text.lines().find(|l| l.starts_with("HVID:")).unwrap_or("");
    // staging: prefer $HOME expansion; cmd fallback via %USERPROFILE% handled by second probe
    let home = id_line.trim_start_matches("HVID:").trim().to_string();
    let staging = if home.starts_with('/') || (shell == DeviceShell::Posix && !home.is_empty()) {
        format!("{}/hvolt-install", home)
    } else {
        return Err(format!("cannot resolve device-host home via probe (got {:?}); windows cmd hosts need bash DefaultShell", id_line));
    };
    Ok(DeviceHost { shell, staging })
}

/// Push a local file THROUGH the device host onto the device, then run a
/// shell command there. Generalizes remote_install's transport (the hqf
/// quickfix flow needs the same two hops with a different device command).
/// Returns the device command's stdout.
pub fn remote_push_and_run(
    spec: &str,
    hdc: &str,
    serial: &str,
    local: &Path,
    device_path: &str,
    device_cmd: &[&str],
) -> Result<String, String> {
    let host = probe_host(spec)?;
    let mk = match host.shell {
        DeviceShell::Posix => format!("mkdir -p '{}'", host.staging),
        DeviceShell::Cmd => format!(
            "mkdir \"{}\" 2>nul || (mkdir \"{}\")",
            host.staging, host.staging
        ),
        DeviceShell::PowerShell => format!("mkdir '{}' -Force | Out-Null", host.staging),
    };
    let out = Command::new("ssh")
        .args(ssh_args(spec, &mk))
        .output()
        .map_err(|e| format!("ssh mkdir: {}", e))?;
    if !out.status.success() {
        return Err(format!(
            "ssh to {} failed: {}",
            spec,
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    let scp = scp_args(spec, local, &host.staging);
    let out = Command::new("scp")
        .args(&scp)
        .output()
        .map_err(|e| format!("scp: {}", e))?;
    if !out.status.success() {
        return Err(format!(
            "scp {} to {} failed: {}",
            local.display(),
            spec,
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    if needs_tconn(serial) {
        let tc = remote_hdc_no_target(hdc, &["tconn", serial], host.shell);
        let _ = Command::new("ssh").args(ssh_args(spec, &tc)).output();
    }
    // staged file on the host
    let name = local
        .file_name()
        .ok_or("no file name")?
        .to_string_lossy()
        .to_string();
    let staged = format!("{}/{}", host.staging, name);
    let send = remote_hdc_cmd(
        hdc,
        serial,
        &["file", "send", &staged, device_path],
        host.shell,
    );
    let out = Command::new("ssh")
        .args(ssh_args(spec, &send))
        .output()
        .map_err(|e| format!("ssh send: {}", e))?;
    let so = String::from_utf8_lossy(&out.stdout).to_string();
    let _ = Command::new("ssh")
        .args(ssh_args(spec, &format!("rm -f '{}'", staged)))
        .output();
    if !so.contains("FileTransfer finish") {
        return Err(format!("device-host file send failed: {}", so.trim()));
    }
    // the device command
    let mut args: Vec<String> = vec!["shell".to_string()];
    args.extend(device_cmd.iter().map(|a| a.to_string()));
    let as_ref: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    let run = remote_hdc_cmd(hdc, serial, &as_ref, host.shell);
    let out = Command::new("ssh")
        .args(ssh_args(spec, &run))
        .output()
        .map_err(|e| format!("ssh run: {}", e))?;
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

/// Install the signed HAP on a REMOTE device host: mkdir → scp → [tconn] → hdc install.
pub fn remote_install(spec: &str, hdc: &str, serial: &str, hap: &Path) -> Result<String, String> {
    let host = probe_host(spec)?;
    // 1. staging dir (absolute — remote single quotes never expand ~)
    let mk = match host.shell {
        DeviceShell::Posix => format!("mkdir -p '{}'", host.staging),
        DeviceShell::Cmd => format!(
            "mkdir \"{}\" 2>nul || (mkdir \"{}\")",
            host.staging, host.staging
        ),
        DeviceShell::PowerShell => format!("mkdir '{}' -Force | Out-Null", host.staging),
    };
    let out = Command::new("ssh")
        .args(ssh_args(spec, &mk))
        .output()
        .map_err(|e| format!("ssh mkdir: {}", e))?;
    if !out.status.success() {
        return Err(format!(
            "ssh to {} failed: {}",
            spec,
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    // 2. push the hap
    let scp = scp_args(spec, hap, &host.staging);
    let out = Command::new("scp")
        .args(&scp)
        .output()
        .map_err(|e| format!("scp: {}", e))?;
    if !out.status.success() {
        return Err(format!(
            "scp {} to {} failed: {}",
            hap.display(),
            spec,
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    // 3. tconn first for network devices
    if needs_tconn(serial) {
        let tc = remote_hdc_no_target(hdc, &["tconn", serial], host.shell);
        let out = Command::new("ssh")
            .args(ssh_args(spec, &tc))
            .output()
            .map_err(|e| format!("ssh tconn: {}", e))?;
        let so = String::from_utf8_lossy(&out.stdout).to_string();
        if !out.status.success() || so.contains("[Fail]") {
            return Err(format!(
                "tconn {} on device host failed: {}",
                serial,
                so.trim()
            ));
        }
    }
    // 4. install
    let name = hap
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let hap_remote = format!("{}/{}", host.staging, name);
    let install = remote_hdc_cmd(hdc, serial, &["install", "-r", &hap_remote], host.shell);
    let out = Command::new("ssh")
        .args(ssh_args(spec, &install))
        .output()
        .map_err(|e| format!("ssh install: {}", e))?;
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    if !out.status.success() || stdout.contains("[Fail]") {
        return Err(format!(
            "remote install failed:\n{}{}",
            stdout.trim(),
            stderr.trim()
        ));
    }
    Ok(stdout)
}

/// hdc command without -t (host-level subcommands like tconn / list).
pub(crate) fn remote_hdc_no_target(hdc: &str, sub: &[&str], shell: DeviceShell) -> String {
    format!("{} {}", quote_arg(hdc, shell), shell_join(sub, shell))
}

/// Launch an ability on the remote device host.
pub fn remote_launch(
    spec: &str,
    hdc: &str,
    serial: &str,
    bundle: &str,
    ability: &str,
) -> Result<String, String> {
    let host = probe_host(spec)?;
    let cmd = remote_hdc_cmd(
        hdc,
        serial,
        &["shell", "aa", "start", "-a", ability, "-b", bundle],
        host.shell,
    );
    let out = Command::new("ssh")
        .args(ssh_args(spec, &cmd))
        .output()
        .map_err(|e| format!("ssh launch: {}", e))?;
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    if !out.status.success() {
        return Err(format!("remote launch failed: {}", stdout.trim()));
    }
    Ok(stdout)
}

#[cfg(test)]
mod tests {
    #[test]
    fn scp_pull_argv_shape() {
        let a = super::scp_pull_args(
            "dev@host.example:50022",
            "/tmp/x.jpeg",
            std::path::Path::new("/local/out.jpeg"),
        );
        let j = a.join(" ");
        assert!(j.contains("-P 50022"), "{}", j);
        assert!(
            j.ends_with("dev@host.example:/tmp/x.jpeg /local/out.jpeg"),
            "{}",
            j
        );
        assert!(j.contains("BatchMode=yes"));
    }
    use super::*;
    use std::path::Path;

    #[test]
    fn host_port_split() {
        assert_eq!(split_host_port("bob@1.2.3.4"), ("bob@1.2.3.4".into(), None));
        assert_eq!(
            split_host_port("bob@1.2.3.4:2222"),
            ("bob@1.2.3.4".into(), Some(2222))
        );
        assert_eq!(
            split_host_port("bob@host.example"),
            ("bob@host.example".into(), None)
        );
    }

    #[test]
    fn scp_argv_shape() {
        let a = scp_args(
            "dev@192.168.1.5:2201",
            Path::new("/tmp/x-signed.hap"),
            "~/hvolt-install",
        );
        assert!(a.windows(2).any(|w| w == ["-P", "2201"]), "{:?}", a);
        assert!(a.iter().any(|s| s == "-o"));
        assert_eq!(
            a.last().unwrap(),
            "dev@192.168.1.5:~/hvolt-install/x-signed.hap"
        );
        // no port → no -P
        let b = scp_args("dev@host", Path::new("/t/a.hap"), "~/d");
        assert!(!b.iter().any(|s| s == "-P"), "{:?}", b);
    }

    #[test]
    fn ssh_argv_shape() {
        let a = ssh_args("dev@host:2222", "echo hi");
        assert!(a.windows(2).any(|w| w == ["-p", "2222"]), "{:?}", a);
        assert_eq!(a.last().unwrap(), "echo hi");
        // host comes immediately before the command
        let i = a
            .iter()
            .position(|s| s == "dev@host")
            .expect("host present");
        assert_eq!(a[i + 1], "echo hi");
        assert!(a.windows(2).any(|w| w == ["-o", "BatchMode=yes"]));
    }

    #[test]
    fn channel_key_injected_into_argv_when_present() {
        let key = Path::new("/fake/hvolt-device-ed25519");
        let a = ssh_args_keyed(Some(key), "dev@host", "echo hi");
        let i = a.iter().position(|s| s == "-i").expect("-i missing");
        assert_eq!(a[i + 1], "/fake/hvolt-device-ed25519");
        // no key → no -i, system defaults
        let b = ssh_args_keyed(None, "dev@host", "echo hi");
        assert!(!b.iter().any(|s| s == "-i"), "{:?}", b);
        let c = scp_args_keyed(Some(key), "dev@host", Path::new("/t/a.hap"), "~/d");
        assert!(c.iter().any(|s| s == "-i"), "scp missing -i: {:?}", c);
    }

    #[test]
    fn remote_hdc_command_quotes() {
        let c = remote_hdc_cmd(
            "/Applications/hdc",
            "ABC123",
            &["install", "-r", "/Users/x/hvolt-install/a.hap"],
            DeviceShell::Posix,
        );
        assert_eq!(
            c,
            "'/Applications/hdc' -t 'ABC123' 'install' '-r' '/Users/x/hvolt-install/a.hap'"
        );
    }

    #[test]
    fn quote_dialects() {
        // cmd: quote when spaces/%
        assert_eq!(
            quote_arg("C:\\Program Files\\hdc.exe", DeviceShell::Cmd),
            "\"C:\\Program Files\\hdc.exe\""
        );
        assert_eq!(quote_arg("hdc", DeviceShell::Cmd), "hdc");
        assert_eq!(quote_arg("a b", DeviceShell::Cmd), "\"a b\"");
        // posix: single-quote everything
        assert_eq!(quote_arg("C:\\x.exe", DeviceShell::Posix), "'C:\\x.exe'");
        // powershell: double the inner singles
        assert_eq!(quote_arg("it's", DeviceShell::PowerShell), "'it''s'");
    }

    #[test]
    fn tconn_detection_for_network_targets() {
        assert!(needs_tconn("192.168.15.6:38565"));
        assert!(!needs_tconn("3DK0124929000096"));
    }
}
