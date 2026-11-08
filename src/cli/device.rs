// Device: wire-protocol exec, emulator, device registry, screenshot, UI dump, logs, files.
use crate::support::cli::{DevAction, EmuAction, FileAction, UiAction};
use std::path::PathBuf;

pub(crate) fn exec(
    addr: String,
    shell: Option<String>,
    put: Option<Vec<String>>,
    install: Option<PathBuf>,
) {
    let timeout = std::time::Duration::from_secs(60);
    let identity = crate::device::hdc_proto::HostIdentity::load_from_home().unwrap_or_else(|e| {
        eprintln!("✗ {}", e);
        std::process::exit(1);
    });
    let result = match (shell, put) {
        (_, Some(paths)) => {
            let local = std::path::PathBuf::from(&paths[0]);
            crate::device::hdc_proto::send_file(
                &addr,
                "Ver: 3.2.0",
                &identity,
                &local,
                &paths[1],
                timeout,
            )
            .map(|()| println!("✓ sent {} -> {}", paths[0], paths[1]))
        }
        (Some(command), None) => {
            crate::device::hdc_proto::shell(&addr, "Ver: 3.2.0", &identity, &command, timeout).map(
                |out| {
                    print!("{}", out);
                },
            )
        }
        (None, None) if install.is_some() => {
            let package = install.expect("checked above");
            crate::device::hdc_proto::install(&addr, "Ver: 3.2.0", &identity, &package, timeout)
                .map(|text| println!("{}", text))
        }
        (None, None) => {
            crate::device::hdc_proto::authenticate(&addr, "Ver: 3.2.0", &identity, timeout).map(
                |report| {
                    println!("host      : {}", report.hostname);
                    println!("device    : {}", report.device_name);
                    println!("auth      : {}", report.auth_status);
                    println!("message   : {}", report.device_message);
                },
            )
        }
    };
    if let Err(e) = result {
        eprintln!("✗ {}", e);
        std::process::exit(1);
    }
}

pub(crate) fn emulator(action: EmuAction) {
    match action {
        crate::support::cli::EmuAction::List { details } => {
            emu(&crate::sdk::emulator::EmuCmd::List { details });
        }
        crate::support::cli::EmuAction::Start { name, no_window } => {
            emu(&crate::sdk::emulator::EmuCmd::Start { name, no_window });
        }
        crate::support::cli::EmuAction::Stop { name } => {
            emu(&crate::sdk::emulator::EmuCmd::Stop { name });
        }
        crate::support::cli::EmuAction::Create {
            name,
            r#type,
            os_version,
            memory,
        } => {
            emu(&crate::sdk::emulator::EmuCmd::Create {
                name,
                device_type: r#type,
                os_version,
                memory,
            });
        }
        crate::support::cli::EmuAction::Delete { name, force } => {
            emu(&crate::sdk::emulator::EmuCmd::Delete { name, force });
        }
        crate::support::cli::EmuAction::Images { action, downloaded } => {
            if downloaded
                && matches!(
                    action,
                    Some(crate::support::cli::ImagesAction::Download { .. })
                )
            {
                eprintln!("note: --downloaded is a list-mode filter; ignored with `download`");
            }
            let cmd = match action {
                Some(crate::support::cli::ImagesAction::Download { os_version, r#type }) => {
                    crate::sdk::emulator::EmuCmd::ImageInstall {
                        device_type: r#type,
                        os_version,
                        force: false,
                    }
                }
                None => crate::sdk::emulator::EmuCmd::Images {
                    downloaded_only: downloaded,
                },
            };
            emu(&cmd);
        }
        crate::support::cli::EmuAction::License => {
            emu(&crate::sdk::emulator::EmuCmd::AcceptLicense);
        }
    }
}

pub(crate) fn device(action: DevAction) {
    match action {
        crate::support::cli::DevAction::List => crate::device::registry::list(),
        crate::support::cli::DevAction::Connect { addr } => crate::device::registry::connect(&addr),
        crate::support::cli::DevAction::Remote { host, serial, hdc } => {
            crate::device::registry::set_remote(&host, &serial, hdc.as_deref().unwrap_or(""))
        }
    }
}

pub(crate) fn screenshot(out: Option<PathBuf>, device: Option<String>) {
    if let Err(e) = crate::device::devtools::screenshot(device.as_deref(), out.as_deref()) {
        eprintln!("✗ {}", e);
        std::process::exit(1);
    }
}

pub(crate) fn ui(action: UiAction) {
    match action {
        crate::support::cli::UiAction::Dump { out, device } => {
            if let Err(e) = crate::device::devtools::ui_dump(device.as_deref(), out.as_deref()) {
                eprintln!("✗ {}", e);
                std::process::exit(1);
            }
        }
    }
}

#[allow(clippy::too_many_arguments)] // one parameter per CLI flag, as in the clap variant
pub(crate) fn log(
    level: Option<String>,
    bundle: Option<String>,
    tag: Option<String>,
    domain: Option<String>,
    keyword: Vec<String>,
    crash: bool,
    follow: bool,
    tail: Option<usize>,
    device: Option<String>,
) {
    let o = crate::device::devtools::LogOpts {
        level,
        tag,
        domain,
        bundle,
        keyword,
        crash,
        follow,
        tail,
    };
    if let Err(e) = crate::device::devtools::log(&o, device.as_deref()) {
        eprintln!("✗ {}", e);
        std::process::exit(1);
    }
}

pub(crate) fn file(action: FileAction) {
    let r = match action {
        crate::support::cli::FileAction::Send {
            local,
            remote,
            device,
        } => crate::device::devtools::file_send(
            std::path::Path::new(&local),
            &remote,
            device.as_deref(),
        ),
        crate::support::cli::FileAction::Recv {
            remote,
            local,
            device,
        } => crate::device::devtools::file_recv(
            &remote,
            std::path::Path::new(&local),
            device.as_deref(),
        ),
    };
    if let Err(e) = r {
        eprintln!("✗ {}", e);
        std::process::exit(1);
    }
}

/// run an emulator action, propagating failure (the OFFICIAL CLI exits 0
/// even when it prints "Device create fail." — swallowing that made every
/// emulator failure look like success to scripts and CI)
fn emu(cmd: &crate::sdk::emulator::EmuCmd) {
    if let Err(e) = crate::sdk::emulator::run(cmd) {
        eprintln!("✗ {}", e);
        std::process::exit(1);
    }
}
