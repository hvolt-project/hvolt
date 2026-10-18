mod apply;
mod assemble;
mod backend;
mod build_hvigor;
mod check;
mod cli;
mod config;
mod device;
mod devtools;
mod doctor;
mod emulator;
mod hap;
mod hdc;
mod http;
mod lint;
mod pipeline;
mod platform;
mod scaffold;
mod sdk;
mod sign;
mod sign_local;
mod target;
mod tools;
mod translator;
mod translator_resources;

/// Report an AGC command that this build cannot serve.
fn agc_failed(r: Result<(), String>) {
    if let Err(e) = r {
        eprintln!("✗ {}", e);
        std::process::exit(2);
    }
}

/// run an emulator action, propagating failure (the OFFICIAL CLI exits 0
/// even when it prints "Device create fail." — swallowing that made every
/// emulator failure look like success to scripts and CI)
fn emu(cmd: &emulator::EmuCmd) {
    if let Err(e) = emulator::run(cmd) {
        eprintln!("✗ {}", e);
        std::process::exit(1);
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

fn main() {
    #[cfg(unix)]
    reset_sigpipe();
    #[cfg(windows)]
    install_pipe_quiet_exit();

    #[cfg(unix)]
    reset_sigpipe();

    let args = cli::parse();
    match args.command {
        cli::Command::Init { name } => {
            // validate the GENERATED bundle segment against the official
            // schema semantics, not the raw name charset — and keep nested
            // paths working (scaffold.plan takes the basename;
            // the previous charset check both missed _abc/abc_ and locked
            // `init examples/test-app`)
            if let Err(e) = scaffold::validate_project_name(&name) {
                eprintln!("✗ {}", e);
                std::process::exit(2);
            }
            scaffold::init(&name);
            println!("✓ Created hvolt project: {}/", name);
            println!("  Next: cd {} && hvolt build", name);
        }
        cli::Command::Assemble => {
            assemble::assemble(std::path::Path::new("."));
        }
        cli::Command::Build => {
            pipeline::build_and_sign(std::path::Path::new(".")).unwrap_or_else(|e| {
                eprintln!("✗ {}", e);
                std::process::exit(1);
            });
        }
        cli::Command::Sign { local, apl, flavor } => {
            if local {
                let kind = match flavor.as_str() {
                    "debug" => sign_local::ProfileKind::Debug,
                    _ => sign_local::ProfileKind::Release,
                };
                if let Err(e) = sign_local::sign(std::path::Path::new("."), &apl, kind) {
                    eprintln!("✗ {}", e);
                    std::process::exit(1);
                }
                return;
            }
            if let Err(e) = backend::sign_remote(std::path::Path::new(".")) {
                eprintln!("✗ {}", e);
                std::process::exit(1);
            }
        }
        cli::Command::Apply { device } => {
            if let Err(e) = apply::apply(std::path::Path::new("."), device.as_deref()) {
                eprintln!("✗ {}", e);
                std::process::exit(1);
            }
        }
        cli::Command::Push {
            device,
            device_flag,
        } => {
            let device = warn_if_conflict(device, device_flag, "push");
            pipeline::push_app(std::path::Path::new("."), device.as_deref()).unwrap_or_else(|e| {
                pipeline::report_push_failure(e);
            });
        }
        cli::Command::Run {
            device,
            device_flag,
        } => {
            let device = warn_if_conflict(device, device_flag, "run");
            pipeline::run(std::path::Path::new("."), device.as_deref());
        }
        cli::Command::Emulator { action } => match action {
            cli::EmuAction::List { details } => {
                emu(&emulator::EmuCmd::List { details });
            }
            cli::EmuAction::Start { name, no_window } => {
                emu(&emulator::EmuCmd::Start { name, no_window });
            }
            cli::EmuAction::Stop { name } => {
                emu(&emulator::EmuCmd::Stop { name });
            }
            cli::EmuAction::Create {
                name,
                r#type,
                os_version,
                memory,
            } => {
                emu(&emulator::EmuCmd::Create {
                    name,
                    device_type: r#type,
                    os_version,
                    memory,
                });
            }
            cli::EmuAction::Delete { name, force } => {
                emu(&emulator::EmuCmd::Delete { name, force });
            }
            cli::EmuAction::Images { action, downloaded } => {
                if downloaded && matches!(action, Some(cli::ImagesAction::Download { .. })) {
                    eprintln!("note: --downloaded is a list-mode filter; ignored with `download`");
                }
                let cmd = match action {
                    Some(cli::ImagesAction::Download { os_version, r#type }) => {
                        emulator::EmuCmd::ImageInstall {
                            device_type: r#type,
                            os_version,
                            force: false,
                        }
                    }
                    None => emulator::EmuCmd::Images {
                        downloaded_only: downloaded,
                    },
                };
                emu(&cmd);
            }
            cli::EmuAction::License => {
                emu(&emulator::EmuCmd::AcceptLicense);
            }
        },
        cli::Command::Device { action } => match action {
            cli::DevAction::List => device::list(),
            cli::DevAction::Connect { addr } => device::connect(&addr),
            cli::DevAction::Remote { host, serial, hdc } => {
                device::set_remote(&host, &serial, &hdc)
            }
        },
        cli::Command::Screenshot { out, device } => {
            if let Err(e) = devtools::screenshot(device.as_deref(), out.as_deref()) {
                eprintln!("✗ {}", e);
                std::process::exit(1);
            }
        }
        cli::Command::Ui { action } => match action {
            cli::UiAction::Dump { out, device } => {
                if let Err(e) = devtools::ui_dump(device.as_deref(), out.as_deref()) {
                    eprintln!("✗ {}", e);
                    std::process::exit(1);
                }
            }
        },
        cli::Command::Log {
            level,
            bundle,
            tag,
            domain,
            keyword,
            crash,
            follow,
            tail,
            device,
        } => {
            let o = devtools::LogOpts {
                level,
                tag,
                domain,
                bundle,
                keyword,
                crash,
                follow,
                tail,
            };
            if let Err(e) = devtools::log(&o, device.as_deref()) {
                eprintln!("✗ {}", e);
                std::process::exit(1);
            }
        }
        cli::Command::File { action } => {
            let r = match action {
                cli::FileAction::Send {
                    local,
                    remote,
                    device,
                } => devtools::file_send(std::path::Path::new(&local), &remote, device.as_deref()),
                cli::FileAction::Recv {
                    remote,
                    local,
                    device,
                } => devtools::file_recv(&remote, std::path::Path::new(&local), device.as_deref()),
            };
            if let Err(e) = r {
                eprintln!("✗ {}", e);
                std::process::exit(1);
            }
        }
        cli::Command::Uninstall { bundle, device } => {
            let b = bundle.unwrap_or_else(|| {
                let c = crate::config::AppConfig::from_file(std::path::Path::new("hvolt.toml"))
                    .unwrap_or_else(|e| {
                        eprintln!("✗ {} — pass the bundle name explicitly", e);
                        std::process::exit(1);
                    });
                c.app.bundle
            });
            if let Err(e) = devtools::uninstall(&b, device.as_deref()) {
                eprintln!("✗ {}", e);
                std::process::exit(1);
            }
        }
        cli::Command::Doctor => {
            let rep = doctor::run(std::path::Path::new("."));
            std::process::exit(rep.finish());
        }
        cli::Command::Check { size_limit } => {
            if let Err(e) = check::check_package(std::path::Path::new("."), size_limit) {
                eprintln!("✗ {}", e);
                std::process::exit(1);
            }
        }
        cli::Command::Lint {
            paths,
            config,
            all_rules,
            json,
            fix,
            fail_on,
            max_defects,
        } => {
            let cfg = config.as_deref().map(std::path::Path::new);
            let opts = lint::LintOptions {
                json,
                fix,
                fail_on: fail_on.unwrap_or(1).clamp(1, 3),
                max_defects: max_defects.unwrap_or(0),
            };
            if let Err(e) =
                lint::lint_with(std::path::Path::new("."), &paths, cfg, all_rules, &opts)
            {
                eprintln!("✗ {}", e);
                std::process::exit(1);
            }
        }
        cli::Command::Hap { sub } => match sub {
            cli::HapCommand::Info { path } => {
                let p = match path {
                    Some(p) => p,
                    None => check::find_signed_hap(std::path::Path::new(".")).unwrap_or_else(|e| {
                        eprintln!("✗ {}", e);
                        std::process::exit(1);
                    }),
                };
                match hap::hap_info(&p) {
                    Ok(text) => {
                        println!("{}", text);
                    }
                    Err(e) => {
                        eprintln!("✗ {}", e);
                        std::process::exit(1);
                    }
                }
            }
        },
        cli::Command::Clean => {
            let build_dir = std::path::Path::new(".").join("build");
            if build_dir.exists() {
                std::fs::remove_dir_all(&build_dir).expect("clean build dir");
                println!("✓ Cleaned build/");
            } else {
                println!("✓ Already clean (no build/)");
            }
        }
        cli::Command::Sdk { action } => match action {
            cli::SdkAction::Info => sdk::show_info(),
            cli::SdkAction::Set { path, global } => sdk::set_sdk(&path, global),
            cli::SdkAction::List => sdk::list_installed(),
            cli::SdkAction::ListRemote => match sdk::list_remote() {
                Ok(versions) => {
                    if versions.is_empty() {
                        println!("No remote SDK versions available.");
                    } else {
                        println!("Available SDK versions:");
                        for v in &versions {
                            let total: u64 = v.components.iter().map(|c| c.size).sum();
                            println!(
                                "  {} — {} ({}, {} total)",
                                v.version,
                                v.display_name,
                                v.release_type,
                                sdk::fmt_size(total)
                            );
                            for c in &v.components {
                                println!("    {:<12} {:>10}", c.path, sdk::fmt_size(c.size));
                            }
                            println!("    api: {}", v.api_version);
                        }
                    }
                }
                Err(e) => eprintln!("✗ {}", e),
            },
            cli::SdkAction::Download {
                version,
                components,
                dry_run,
            } => {
                sdk::download_sdk(&version, &components, dry_run).unwrap_or_else(|e| {
                    eprintln!("✗ Failed to download SDK: {}", e);
                    std::process::exit(1);
                });
            }
            cli::SdkAction::Import { path } => {
                sdk::import_sdk(&path).unwrap_or_else(|e| {
                    eprintln!("✗ Failed to import SDK: {}", e);
                    std::process::exit(1);
                });
            }
            cli::SdkAction::Use { version } => {
                sdk::use_version(&version).unwrap_or_else(|e| {
                    eprintln!("✗ Failed to switch SDK version: {}", e);
                    std::process::exit(1);
                });
            }
            cli::SdkAction::Remove { version, yes } => {
                if !yes {
                    eprintln!("✗ refusing to delete SDK {} without --yes (irreversible: re-download is ~1GB)", version);
                    eprintln!("  confirm with: hvolt sdk remove {} --yes", version);
                    std::process::exit(2);
                }
                sdk::remove_version(&version).unwrap_or_else(|e| {
                    eprintln!("✗ Failed to remove SDK version: {}", e);
                    std::process::exit(1);
                });
            }
        },
        cli::Command::Agc { action } => match action {
            cli::AgcAction::Login => match backend::login() {
                Ok(()) => println!("✓ Login successful."),
                Err(e) => {
                    eprintln!("✗ Login failed: {}", e);
                    std::process::exit(1);
                }
            },
            cli::AgcAction::Certs { action: None } => agc_failed(backend::list_certs()),
            cli::AgcAction::Certs {
                action: Some(cli::CertAction::Delete { id, yes }),
            } => {
                if !yes {
                    eprintln!("✗ refusing to delete certificate {} without --yes (server-side, irreversible)", id);
                    eprintln!("  confirm with: hvolt agc certs delete {} --yes", id);
                    std::process::exit(2);
                }
                agc_failed(backend::delete_cert(&id))
            }
            cli::AgcAction::Devices => agc_failed(backend::list_devices()),
            cli::AgcAction::Whoami => agc_failed(backend::whoami()),
        },
        cli::Command::Tools { action } => match action {
            cli::ToolsAction::List => tools::list_tools(),
            cli::ToolsAction::ListRemote => {
                tools::list_remote().unwrap_or_else(|e| {
                    eprintln!("✗ {}", e);
                    std::process::exit(1);
                });
            }
            cli::ToolsAction::Install {
                tool,
                version,
                yes,
                from,
            } => {
                let result = match tool.as_str() {
                    "hvigor" => tools::install_hvigor(version.as_deref()),
                    "ohpm" => tools::install_ohpm(version.as_deref(), yes),
                    "codelinter" => tools::install_codelinter(from.as_deref()),
                    _ => Err(format!(
                        "Unknown tool '{}'. Available: hvigor, ohpm, codelinter",
                        tool
                    )),
                };
                if let Err(e) = result {
                    eprintln!("✗ {}", e);
                    std::process::exit(1);
                }
            }
        },
        cli::Command::Config { action } => match action {
            cli::ConfigAction::Get => sdk::config_get(),
            cli::ConfigAction::Set { key, value } => {
                sdk::config_set(&key, &value).unwrap_or_else(|e| {
                    eprintln!("✗ {}", e);
                    std::process::exit(1);
                });
            }
            cli::ConfigAction::Unset { key } => {
                sdk::config_unset(&key).unwrap_or_else(|e| {
                    eprintln!("✗ {}", e);
                    std::process::exit(1);
                });
            }
        },
    }
}

/// CLI convention: a broken stdout pipe (head/less closing early) exits
/// silently with the default SIGPIPE action instead of a Rust panic.
#[cfg(unix)]
fn reset_sigpipe() {
    // The only `unsafe` in the crate: restoring the default SIGPIPE action so
    // `hvolt ... | head` exits quietly instead of panicking on EPIPE.
    #[allow(unsafe_code)]
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }
}

/// Windows has no SIGPIPE: ERROR_PIPE_BUSY(232) surfaces as a println!
/// panic. Same CLI convention: a closed pipe means "consumer done",
/// exit silently.
#[cfg(windows)]
fn install_pipe_quiet_exit() {
    std::panic::set_hook(Box::new(|info| {
        let msg = info.to_string();
        if msg.contains("failed printing to stdout") {
            std::process::exit(0);
        }
        eprintln!("{}", msg);
        std::process::exit(101);
    }));
}
