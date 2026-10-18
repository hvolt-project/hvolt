// device: manage connected devices. Wraps `hdc list targets`, persists the
// chosen install target via `hvolt target` semantics in target.rs.

use std::path::PathBuf;
use std::process::Command;

fn find_hdc() -> Option<PathBuf> {
    crate::target::find_hdc_unified()
}

pub fn list() {
    let hdc = find_hdc().unwrap_or_else(|| {
        eprintln!(
            "✗ hdc not found (PATH or SDK toolchains) — `hvolt sdk download <version>` provides it"
        );
        std::process::exit(1);
    });
    // Use -v for detailed info
    let status = Command::new(&hdc)
        .arg("list")
        .arg("targets")
        .arg("-v")
        .status()
        .unwrap_or_else(|e| {
            eprintln!("✗ cannot run hdc: {}", e);
            std::process::exit(1);
        });
    if !status.success() {
        // a failed listing is a failed command — printing the install
        // target below and exiting 0 would report success without installing
        eprintln!("✗ hdc list targets failed");
        std::process::exit(1);
    }
    let cfg = crate::sdk::load_global_config();
    if !cfg.device_target.is_empty() {
        println!(
            "install target: {} (from ~/.hvolt/config.json)",
            cfg.device_target
        );
    } else {
        println!("install target: (none set — run `hvolt device connect <serial|ip:port>`)");
    }
}

/// TCP connect and/or record serial as the persistent install target.
pub fn connect(addr: &str) {
    crate::target::connect_and_save(addr).unwrap_or_else(|e| {
        eprintln!("✗ Failed to connect: {}", e);
        std::process::exit(1);
    });
}

/// Register a cross-machine install target: build host ships HAP via scp and
/// drives the device through the device host's hdc.
pub fn set_remote(host: &str, serial: &str, hdc: &str) {
    // ensure hvolt's dedicated channel key exists (never silently generated before)
    let key = crate::target::device_key_path();
    if !key.exists() {
        std::process::Command::new("ssh-keygen")
            .args([
                "-t",
                "ed25519",
                "-f",
                &key.to_string_lossy(),
                "-N",
                "",
                "-C",
                "hvolt-device-channel",
                "-q",
            ])
            .status()
            .ok();
    }
    // connectivity precheck: fail early with a specific error
    let probe = crate::target::ssh_args(host, "echo hvolt-ok");
    let out = std::process::Command::new("ssh").args(&probe).output();
    match out {
        Ok(o) if o.status.success() && String::from_utf8_lossy(&o.stdout).contains("hvolt-ok") => {}
        Ok(o) => {
            eprintln!(
                "✗ ssh {} unreachable: {}",
                host,
                String::from_utf8_lossy(&o.stderr).trim()
            );
            eprintln!(
                "  1) enable Remote Login on the device host (System Settings → General → Sharing)"
            );
            eprintln!("  2) trust this channel public key on the device host:");
            if let Ok(pubk) = std::fs::read_to_string(format!("{}.pub", key.display())) {
                eprintln!("     echo '{}' >> ~/.ssh/authorized_keys", pubk.trim());
            }
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("✗ ssh probe failed: {}", e);
            std::process::exit(1);
        }
    }
    let mut cfg = crate::sdk::load_global_config();
    cfg.device_host = host.to_string();
    cfg.device_target = serial.to_string();
    cfg.device_hdc = hdc.to_string();
    crate::sdk::save_global_config(&cfg).unwrap_or_else(|e| {
        eprintln!("✗ {}", e);
        std::process::exit(1);
    });
    println!(
        "✓ Remote install target set: {} serial={} hdc={}",
        host, serial, hdc
    );
}
