// AGC: `hvolt agc` subcommands — login, certificates, devices, account.
use crate::support::cli::AgcAction;

pub(crate) fn agc(action: AgcAction) {
    match action {
        crate::support::cli::AgcAction::Login => match crate::sign::backend::login() {
            Ok(()) => println!("✓ Login successful."),
            Err(e) => {
                eprintln!("✗ Login failed: {}", e);
                std::process::exit(1);
            }
        },
        crate::support::cli::AgcAction::Certs { action: None } => {
            agc_failed(crate::sign::backend::list_certs())
        }
        crate::support::cli::AgcAction::Certs {
            action: Some(crate::support::cli::CertAction::Delete { id, yes }),
        } => {
            if !yes {
                eprintln!(
                    "✗ refusing to delete certificate {} without --yes (server-side, irreversible)",
                    id
                );
                eprintln!("  confirm with: hvolt agc certs delete {} --yes", id);
                std::process::exit(2);
            }
            agc_failed(crate::sign::backend::delete_cert(&id))
        }
        crate::support::cli::AgcAction::Devices => agc_failed(crate::sign::backend::list_devices()),
        crate::support::cli::AgcAction::Device { action } => match action {
            crate::support::cli::AgcDeviceAction::Add { udid, name } => {
                agc_failed(crate::sign::backend::add_device(&udid, name.as_deref()))
            }
            crate::support::cli::AgcDeviceAction::Delete { id } => {
                agc_failed(crate::sign::backend::delete_device(&id))
            }
        },
        crate::support::cli::AgcAction::Whoami => agc_failed(crate::sign::backend::whoami()),
    }
}

/// Report an AGC command that this build cannot serve.
fn agc_failed(r: Result<(), String>) {
    if let Err(e) = r {
        eprintln!("✗ {}", e);
        std::process::exit(2);
    }
}
