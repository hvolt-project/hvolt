// Deploy: push, run, uninstall and HAP introspection after a build.
use crate::support::cli::HapCommand;
use std::path::PathBuf;

pub(crate) fn push(device: Option<String>, device_flag: Option<String>, hap: Option<PathBuf>) {
    let device = warn_if_conflict(device, device_flag, "push");
    let result = match hap {
        Some(package) => crate::build::deps::pipeline::push_package(&package, device.as_deref()),
        None => {
            crate::build::deps::pipeline::push_app(std::path::Path::new("."), device.as_deref())
        }
    };
    result.unwrap_or_else(|e| crate::build::deps::pipeline::report_push_failure(e));
}

pub(crate) fn run(device: Option<String>, device_flag: Option<String>) {
    let device = warn_if_conflict(device, device_flag, "run");
    crate::build::deps::pipeline::run(std::path::Path::new("."), device.as_deref());
}

pub(crate) fn uninstall(bundle: Option<String>, device: Option<String>) {
    let b = bundle.unwrap_or_else(|| {
        let c = crate::support::config::AppConfig::from_file(std::path::Path::new("hvolt.toml"))
            .unwrap_or_else(|e| {
                eprintln!("✗ {} — pass the bundle name explicitly", e);
                std::process::exit(1);
            });
        c.app.bundle
    });
    if let Err(e) = crate::device::devtools::uninstall(&b, device.as_deref()) {
        eprintln!("✗ {}", e);
        std::process::exit(1);
    }
}

pub(crate) fn hap(sub: HapCommand) {
    match sub {
        crate::support::cli::HapCommand::Info { path } => {
            let p = match path {
                Some(p) => p,
                None => crate::project::check::find_signed_hap(std::path::Path::new("."))
                    .unwrap_or_else(|e| {
                        eprintln!("✗ {}", e);
                        std::process::exit(1);
                    }),
            };
            match crate::project::hap::hap_info(&p) {
                Ok(text) => {
                    println!("{}", text);
                }
                Err(e) => {
                    eprintln!("✗ {}", e);
                    std::process::exit(1);
                }
            }
        }
    }
}

/// positional + flag both given → honor positional, say so (never silently)
fn warn_if_conflict(pos: Option<String>, flag: Option<String>, cmd: &str) -> Option<String> {
    match (pos, flag) {
        (Some(p), Some(f)) => {
            eprintln!(
                "note: both positional and -d given to {}; using positional '{}', ignoring '{}'",
                cmd, p, f
            );
            Some(p)
        }
        (p, f) => p.or(f),
    }
}
