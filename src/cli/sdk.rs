// SDK: `hvolt sdk` subcommands — info, paths, versions, download, import, use, remove.
use crate::support::cli::SdkAction;

pub(crate) fn sdk(action: SdkAction) {
    match action {
        crate::support::cli::SdkAction::Info => crate::sdk::manage::show_info(),
        crate::support::cli::SdkAction::Set { path, global } => {
            crate::sdk::manage::set_sdk(&path, global)
        }
        crate::support::cli::SdkAction::List => crate::sdk::manage::list_installed(),
        crate::support::cli::SdkAction::ListRemote => match crate::sdk::manage::list_remote() {
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
                            crate::sdk::manage::fmt_size(total)
                        );
                        for c in &v.components {
                            println!(
                                "    {:<12} {:>10}",
                                c.path,
                                crate::sdk::manage::fmt_size(c.size)
                            );
                        }
                        println!("    api: {}", v.api_version);
                    }
                }
            }
            Err(e) => eprintln!("✗ {}", e),
        },
        crate::support::cli::SdkAction::Download {
            version,
            components,
            dry_run,
        } => {
            crate::sdk::manage::download_sdk(&version, &components, dry_run).unwrap_or_else(|e| {
                eprintln!("✗ Failed to download SDK: {}", e);
                std::process::exit(1);
            });
        }
        crate::support::cli::SdkAction::Import { path } => {
            crate::sdk::manage::import_sdk(&path).unwrap_or_else(|e| {
                eprintln!("✗ Failed to import SDK: {}", e);
                std::process::exit(1);
            });
        }
        crate::support::cli::SdkAction::Use { version } => {
            crate::sdk::manage::use_version(&version).unwrap_or_else(|e| {
                eprintln!("✗ Failed to switch SDK version: {}", e);
                std::process::exit(1);
            });
        }
        crate::support::cli::SdkAction::Remove { version, yes } => {
            if !yes {
                eprintln!(
                    "✗ refusing to delete SDK {} without --yes (irreversible: re-download is ~1GB)",
                    version
                );
                eprintln!("  confirm with: hvolt sdk remove {} --yes", version);
                std::process::exit(2);
            }
            crate::sdk::manage::remove_version(&version).unwrap_or_else(|e| {
                eprintln!("✗ Failed to remove SDK version: {}", e);
                std::process::exit(1);
            });
        }
    }
}
