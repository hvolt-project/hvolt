// device: manage connected devices. USB devices are enumerated natively
// (nusb, HDCD bulk interface); the chosen install target is persisted via
// target.rs semantics.

/// Debug detail dump of every interface on the HDCD device (class/subclass).
fn print_usb_detail(serial: &str) {
    use nusb::MaybeFuture;
    if let Ok(list) = nusb::list_devices().wait() {
        for d in list.filter(|d| d.serial_number() == Some(serial)) {
            for i in d.interfaces() {
                println!(
                    "  iface {}: class=0x{:02x} subclass=0x{:02x} protocol=0x{:02x}",
                    i.interface_number(),
                    i.class(),
                    i.subclass(),
                    i.protocol()
                );
            }
        }
    }
}

pub fn list() {
    // Native USB enumeration (nusb): HDCD interfaces over bulk endpoints.
    // No external hdc is spawned.
    let usb = crate::device::usb::enumerate_hdcd();
    if usb.is_empty() {
        println!("(no device over USB)");
    } else {
        for d in &usb {
            if std::env::var("HVOLT_USB_DEBUG").is_ok() {
                print_usb_detail(&d.serial);
            }
            println!("{}\t\tUSB\tConnected\tlocalhost", d.serial);
        }
    }
    let cfg = crate::sdk::manage::load_global_config();
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
    crate::device::target::connect_and_save(addr).unwrap_or_else(|e| {
        eprintln!("✗ Failed to connect: {}", e);
        std::process::exit(1);
    });
}

/// Register a cross-machine install target: build host ships HAP via scp and
/// drives the device through the device host's hdc.
pub fn set_remote(host: &str, serial: &str, hdc: &str) {
    // ensure hvolt's dedicated channel key exists (never silently generated before)
    let key = crate::device::target::device_key_path();
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
    let out = crate::device::target::ssh_run(host, "echo hvolt-ok");
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
    let mut cfg = crate::sdk::manage::load_global_config();
    cfg.device_host = host.to_string();
    cfg.device_target = serial.to_string();
    cfg.device_hdc = hdc.to_string();
    crate::sdk::manage::save_global_config(&cfg).unwrap_or_else(|e| {
        eprintln!("✗ {}", e);
        std::process::exit(1);
    });
    println!(
        "✓ Remote install target set: {} serial={} hdc={}",
        host, serial, hdc
    );
}
