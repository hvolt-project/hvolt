// Config and skill: `hvolt config` values and the embedded-document commands.
use crate::support::cli::{ConfigAction, SkillAction};

pub(crate) fn config(action: ConfigAction) {
    match action {
        crate::support::cli::ConfigAction::Get => crate::sdk::manage::config_get(),
        crate::support::cli::ConfigAction::Set { key, value } => {
            crate::sdk::manage::config_set(&key, &value).unwrap_or_else(|e| {
                eprintln!("✗ {}", e);
                std::process::exit(1);
            });
        }
        crate::support::cli::ConfigAction::Unset { key } => {
            crate::sdk::manage::config_unset(&key).unwrap_or_else(|e| {
                eprintln!("✗ {}", e);
                std::process::exit(1);
            });
        }
    }
}

pub(crate) fn skill(action: SkillAction) {
    match action {
        crate::support::cli::SkillAction::List { plain, json } => {
            if plain {
                crate::support::skill::print_list_plain();
            } else if json {
                crate::support::skill::print_list_json();
            } else {
                crate::support::skill::print_list();
            }
        }
        crate::support::cli::SkillAction::Show { name } => {
            if !crate::support::skill::print_skill(&name) {
                eprintln!("✗ unknown skill: {}", name);
                eprintln!("  available: {}", crate::support::skill::ids().join(", "));
                std::process::exit(1);
            }
        }
    }
}
