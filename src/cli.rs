use std::path::PathBuf;

use clap::{Parser, Subcommand};

// `concat!` accepts literals only, so the SDK stamp below cannot reference
// `config::SDK_PLATFORM_VERSION`; `version_stamp_matches_the_config_constant` reads this file
// and fails the suite when the two drift.

#[derive(Parser)]
#[command(
    name = "hvolt",
    about = "HarmonyOS community build tool",
    version = env!("CARGO_PKG_VERSION"),
    long_version = concat!(env!("CARGO_PKG_VERSION"), " (SDK ", "26.0.0", ")"),
    after_help = "Examples:\n  \
        hvolt init myapp && cd myapp && hvolt run        # project → device in one line\n  \
        hvolt sdk list-remote && hvolt sdk download 26.0.0.38\n  \
        hvolt check --size-limit 2M                       # package quality gate\n  \
        hvolt screenshot -d <serial>                      # capture from a specific device\n\nDevEco Studio is NOT required, except for `lint` (needs its codelinter payload)\nand `emulator` (needs its binary and images). See README § DevEco Studio."
)]
pub struct Args {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Create a new hvolt project
    Init {
        /// Project name
        name: String,
    },
    /// Import: DevEco Studio layout → flat hvolt project (reverse of assemble)
    Import {
        /// Project to import (default: current directory)
        #[arg(long, value_name = "DIR")]
        from: Option<PathBuf>,
        /// Where to write the flat project (default: <cwd>/<name>-flat)
        #[arg(long, value_name = "DIR")]
        out: Option<PathBuf>,
        /// Only report the mapping and the fields the flat layout cannot express
        #[arg(long)]
        dry_run: bool,
    },
    /// Assemble: translate user structure → HarmonyOS standard layout
    Assemble,
    /// Build: assemble + invoke hvigorw + sign
    Build {
        /// Put ohpm/hvigor caches here instead of $HOME (containers, CI, sandboxes)
        #[arg(long, value_name = "DIR")]
        cache_dir: Option<PathBuf>,
    },
    /// Sign: generate signing materials (.p12, .cer, .p7b) and configure project
    #[command(
        after_help = "`--local` works in every build. Remote signing through AppGallery Connect\n\
        is provided by a backend that is not part of this source tree; a build\n\
        configured with it (cargo feature `agc-local`) enables `hvolt sign` without\n\
        `--local` and the `hvolt agc` commands. Without it those commands explain\n\
        that the backend is missing."
    )]
    Sign {
        /// OpenHarmony localSign with the official public test chain
        /// (dev boards / emulators / source-built ROMs; NOT retail devices)
        #[arg(long)]
        local: bool,
        /// localSign apl level (normal|system_basic|system_core)
        #[arg(long, default_value = "normal", value_parser = ["normal", "system_basic", "system_core"])]
        apl: String,
        /// localSign certificate flavor (debug|release)
        #[arg(long, default_value = "release", value_parser = ["debug", "release"])]
        flavor: String,
    },
    /// Quickfix incremental deploy (hqf) — patch the running package without
    /// reinstall; requires the app already installed with the SAME identity
    Apply {
        /// Device address (serial or ip:port)
        #[arg(short, long)]
        device: Option<String>,
    },
    /// Push signed.hap to device (default: newest output/*-signed.hap)
    Push {
        /// Device address (e.g. 192.168.1.100:38565 or serial number)
        device: Option<String>,
        /// Target device (same as positional; for flag-style muscle memory)
        #[arg(short = 'd', long = "device", value_name = "DEVICE")]
        device_flag: Option<String>,
    },
    /// Build + sign + push + launch in one shot
    Run {
        /// Device address (e.g. 192.168.1.100:38565 or serial number)
        device: Option<String>,
        /// Target device (same as positional)
        #[arg(short = 'd', long = "device", value_name = "DEVICE")]
        device_flag: Option<String>,
        /// Put ohpm/hvigor caches here instead of $HOME
        #[arg(long, value_name = "DIR")]
        cache_dir: Option<PathBuf>,
    },
    /// Package-side quality gate (official app_check_tool engine)
    Check {
        /// also flag files over this size (bytes, or 50K / 2M / 1G, case-insensitive)
        #[arg(long, value_parser = parse_size)]
        size_limit: Option<u64>,
    },
    /// Language server for editors (LSP over stdio)
    #[command(
        after_help = "ArkTS semantics come from the TypeScript build inside the installed SDK\n\
        (see lsp/DESIGN.md). Consumed by an editor extension, not by hand:\n\
        \n\
        \x20 VS Code (or any LSP client) spawns `hvolt lsp --stdio`.\n\
        \x20 Requires an installed SDK (hvolt sdk download) and Node.js."
    )]
    Lsp {
        /// Speak LSP on stdin/stdout (the only supported transport)
        #[arg(long)]
        stdio: bool,
        /// Override the SDK the language service loads its TypeScript from
        #[arg(long)]
        sdk: Option<String>,
    },
    /// Code Linter (DevEco's official codelinter engine) over the translated tree
    #[command(
        after_help = "This one command needs a DevEco Studio artifact: Huawei ships the\n\
        codelinter payload only inside DevEco Studio (npm and the SDK component\n\
        list do not carry it — measured). Copy it once from a machine that has\n\
        DevEco, then `hvolt lint` runs on a DevEco-free machine:\n\
        \n\
        \x20 hvolt tools install codelinter --from <DevEco>/plugins/codelinter"
    )]
    Lint {
        /// Paths to check (default: the whole translated tree). Each path
        /// must live inside the project.
        paths: Vec<String>,
        /// Use this codelinter config instead of hvolt's own
        #[arg(long)]
        config: Option<String>,
        /// Use the broad rule sets (@performance/all + @typescript-eslint/all).
        /// The official default ("recommended") stays silent on defects these
        /// sets flag (measured).
        #[arg(long)]
        all_rules: bool,
        /// Machine-readable output: one JSON document on stdout (human detail
        /// goes away, stderr keeps the reason on failure).
        #[arg(long)]
        json: bool,
        /// Apply codelinter's fixes IN PLACE. Requires explicit paths: fixes
        /// written into the managed tree copy would be thrown away.
        #[arg(long)]
        fix: bool,
        /// Check only the files handed over by `--overlay-file` (editor path).
        /// A whole-tree run costs seconds; the changed files cost a fraction.
        #[arg(long)]
        scoped: bool,
        /// Fail when a defect at or above this severity exists (1 = most
        /// severe, 2 = warning, 3 = info). Default 1.
        #[arg(long, value_name = "SEV")]
        fail_on: Option<u8>,
        /// Tolerate up to N qualifying defects (default 0 = any fails).
        #[arg(long, value_name = "N")]
        max_defects: Option<usize>,
        /// Check the contents in this file instead of what is on disk. It must
        /// hold a JSON object mapping a source path to its text — an editor's
        /// unsaved buffers, so lint reflects what is on screen rather than the
        /// last save. A file rather than inline JSON: a buffer is easily 100 KB.
        #[arg(long, value_name = "FILE")]
        overlay_file: Option<std::path::PathBuf>,
    },
    /// HAP introspection straight from the package
    Hap {
        #[command(subcommand)]
        sub: HapCommand,
    },
    /// Clean build artifacts
    Clean,
    /// Device screenshot (jpeg — the device capture format)
    Screenshot {
        /// output file (default: screenshot-<ts>.jpeg)
        #[arg(short, long)]
        out: Option<std::path::PathBuf>,
        /// target device (serial or ip:port)
        #[arg(short, long)]
        device: Option<String>,
    },
    /// ArkUI layout tree dump (uitest dumpLayout)
    Ui {
        #[command(subcommand)]
        action: UiAction,
    },
    /// Stream or dump device hilog with filters
    Log {
        /// Log level filter: D|I|W|E|F (comma list)
        #[arg(long)]
        level: Option<String>,
        /// Filter by bundle name (matches the process column)
        #[arg(long)]
        bundle: Option<String>,
        /// hilog tag filter, comma-separated (max 10)
        #[arg(short = 'T', long)]
        tag: Option<String>,
        /// hilog domain filter, comma-separated (max 5)
        #[arg(short = 'D', long)]
        domain: Option<String>,
        /// Keyword filter (repeatable; all must match)
        #[arg(short = 'k', long)]
        keyword: Vec<String>,
        /// Crash logs only (FAULT domain at error level)
        #[arg(long)]
        crash: bool,
        /// Follow the log stream instead of one-shot dump
        #[arg(short = 'f', long)]
        follow: bool,
        /// Keep only the last N matching lines
        #[arg(long)]
        tail: Option<usize>,
        /// Target device (default: configured target)
        #[arg(short = 'd', long)]
        device: Option<String>,
    },
    /// File transfer with the device
    File {
        #[command(subcommand)]
        action: FileAction,
    },
    /// Uninstall an app from the device
    Uninstall {
        /// Bundle name (default: from hvolt.toml)
        bundle: Option<String>,
        /// Target device (default: configured target)
        #[arg(short = 'd', long)]
        device: Option<String>,
    },
    /// Check environment, SDK, tools, device, auth and project consistency
    Doctor,
    /// Manage emulators
    #[command(
        after_help = "The emulator binary and system images ship only with DevEco Studio\n\
        (Windows/macOS). Linux has no official emulator binary — use a physical\n\
        device there."
    )]
    Emulator {
        #[command(subcommand)]
        action: EmuAction,
    },
    /// Manage connected devices
    Device {
        #[command(subcommand)]
        action: DevAction,
    },
    /// Manage HarmonyOS SDK
    Sdk {
        #[command(subcommand)]
        action: SdkAction,
    },
    /// Manage AppGallery Connect resources (certs, devices)
    Agc {
        #[command(subcommand)]
        action: AgcAction,
    },
    /// Manage build tools (node, hvigor, ohpm, JDK)
    Tools {
        #[command(subcommand)]
        action: ToolsAction,
    },
    /// Manage hvolt configuration (java_home, node_home, sdk_path)
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    /// Show documentation embedded in the binary (hvolt-skill, clean-code, harmony-device-flow)
    Skill {
        #[command(subcommand)]
        action: SkillAction,
    },
}

/// `hvolt skill` subcommands.
#[derive(Subcommand)]
pub enum SkillAction {
    /// List embedded documents
    List {
        /// One id per line — nothing else (agent-friendly)
        #[arg(long)]
        plain: bool,
        /// JSON array of {id, description}
        #[arg(long)]
        json: bool,
    },
    /// Print one embedded document
    Show {
        /// Document id (see `hvolt skill list`)
        name: String,
    },
}

#[derive(Subcommand)]
pub enum AgcAction {
    /// Login to Huawei AppGallery Connect (opens browser)
    Login,
    /// List or delete certificates
    Certs {
        #[command(subcommand)]
        action: Option<CertAction>,
    },
    /// List registered devices
    Devices,
    /// Show current login user info
    Whoami,
}

#[derive(Subcommand)]
pub enum CertAction {
    /// Delete a certificate by ID (irreversible server-side operation)
    Delete {
        /// Certificate ID
        id: String,
        /// actually delete (without it the command refuses)
        #[arg(long)]
        yes: bool,
    },
}

#[derive(Subcommand)]
pub enum SdkAction {
    /// Show current SDK configuration and detected paths
    Info,
    /// Set SDK path for current project (or --global for all projects)
    Set {
        /// SDK path
        path: String,
        /// Set globally (write to ~/.hvolt/config.json)
        #[arg(long)]
        global: bool,
    },
    /// List installed SDK versions
    List,
    /// List available SDK versions from remote
    ListRemote,
    /// Download and install a specific SDK version
    Download {
        /// SDK version to download (e.g. 26.0.0.23)
        version: String,
        /// Components to install (ets,js,native,previewer,toolchains; default all)
        #[arg(long, value_delimiter = ',')]
        components: Vec<String>,
        /// Report components and sizes only; download nothing
        #[arg(long)]
        dry_run: bool,
    },
    /// Switch to a specific SDK version
    Use {
        /// SDK version to use (e.g. 26.0.0.23)
        version: String,
    },
    /// Import an SDK version directory into ~/.hvolt/sdk/
    Import {
        /// Path to an SDK version directory (containing sdk-pkg.json)
        path: String,
    },
    /// Remove an installed SDK version from ~/.hvolt/sdk/
    Remove {
        /// SDK version to remove (e.g. 26.0.0.23)
        version: String,
        /// actually delete (without it the command refuses — irreversible op)
        #[arg(long)]
        yes: bool,
    },
}

#[derive(Subcommand)]

pub enum EmuAction {
    /// List available emulator instances
    List {
        /// include image/abi/state details
        #[arg(long)]
        details: bool,
    },
    /// Start an emulator instance
    Start {
        /// emulator instance name (see `hvolt emulator list`)
        name: String,
        /// headless start (CI / no display)
        #[arg(long)]
        no_window: bool,
    },
    /// Stop an emulator instance
    Stop {
        /// emulator instance name (see `hvolt emulator list`)
        name: String,
    },
    /// Create an emulator instance (needs a downloaded image)
    Create {
        name: String,
        /// official device type catalogue (measured -imageList)
        #[arg(long, default_value = "phone", value_parser = ["phone","tablet","2in1","2in1 Foldable","foldable","widefold","triplefold","tv","wearable","wearablekid"])]
        r#type: String,
        /// os version as listed by `emulator images` (the official CLI
        /// REQUIRES it: "The virtual device version cannot be empty")
        #[arg(long, required = true)]
        os_version: Option<String>,
        /// memory in MB
        #[arg(long)]
        memory: Option<u32>,
    },
    /// Delete an emulator instance
    Delete {
        /// emulator instance name (see `hvolt emulator list`)
        name: String,
        /// skip the "is it running?" guard
        #[arg(long)]
        force: bool,
    },
    /// System images: plain lists; `images download <ver>` fetches one
    Images {
        #[command(subcommand)]
        action: Option<ImagesAction>,
        /// only already-downloaded (list mode)
        #[arg(long)]
        downloaded: bool,
    },
    /// Accept the HarmonyOS emulator software agreement
    License,
}

#[derive(Subcommand)]
pub enum DevAction {
    /// List connected devices
    List,
    /// Connect to a device via TCP
    Connect { addr: String },
    /// Register a cross-machine install target (device attached to another host)
    Remote {
        /// Device host, e.g. user@192.168.1.5 or user@host:2201
        host: String,
        /// Device serial on that host (hdc list targets)
        serial: String,
        /// Absolute path of hdc on the device host
        #[arg(long)]
        hdc: String,
    },
}

#[derive(Subcommand, Clone)]
pub enum ToolsAction {
    /// List installed/available tools
    List,
    /// Install a build tool (hvigor, ohpm, codelinter)
    Install {
        tool: String,
        /// Explicit remote version (default: hvolt's verified pin; see `hvolt tools list-remote`)
        #[arg(long)]
        version: Option<String>,
        /// Payload directory for tools that are not remotely fetchable (codelinter):
        /// pass `plugins/codelinter` from an install of DevEco Studio
        #[arg(long, value_name = "DIR")]
        from: Option<String>,
        /// Skip the size-confirmation gate for multi-GB platform packages
        #[arg(long)]
        yes: bool,
    },
    /// List remotely available tool versions (npm registry + mirror)
    ListRemote,
}
pub fn parse() -> Args {
    Args::parse()
}

#[derive(Subcommand)]
pub enum ConfigAction {
    /// Show all configuration values
    Get,
    /// Set a configuration key: sdk_path, node_home, java_home, ohpm_path,
    /// sdk_support_version, device_target, device_host, device_hdc
    Set {
        /// Configuration key
        key: String,
        /// Configuration value
        value: String,
    },
    /// Unset a configuration key
    Unset {
        /// Configuration key to remove
        key: String,
    },
}

#[derive(Subcommand)]
pub enum FileAction {
    /// Copy a file onto the device
    Send {
        /// path on this machine
        local: String,
        /// path on the device (e.g. /data/local/tmp/x)
        remote: String,
        /// Target device (default: configured target)
        #[arg(short = 'd', long)]
        device: Option<String>,
    },
    /// Copy a file from the device
    Recv {
        /// path on the device
        remote: String,
        /// path on this machine
        local: String,
        /// Target device (default: configured target)
        #[arg(short = 'd', long)]
        device: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum HapCommand {
    /// bundle/version/abilities/permissions read from the package itself
    Info {
        /// path to .hap (default: newest output/*-signed.hap)
        path: Option<std::path::PathBuf>,
    },
}

#[derive(Subcommand)]
pub enum UiAction {
    /// dump the current layout hierarchy to a json file
    Dump {
        /// output file (default: layout-<ts>.json)
        #[arg(short, long)]
        out: Option<std::path::PathBuf>,
        /// target device (serial or ip:port)
        #[arg(short, long)]
        device: Option<String>,
    },
}

/// Human sizes: "50000" | "50K" | "2M" | "1G" (case-insensitive, K=1024).
fn parse_size(v: &str) -> Result<u64, String> {
    let t = v.trim().to_uppercase();
    let (num, mult) = match t.strip_suffix('K') {
        Some(n) => (n, 1024u64),
        None => match t.strip_suffix('M') {
            Some(n) => (n, 1024 * 1024),
            None => match t.strip_suffix('G') {
                Some(n) => (n, 1024 * 1024 * 1024),
                None => (t.as_str(), 1),
            },
        },
    };
    num.trim()
        .parse::<u64>()
        .map(|x| x * mult)
        .map_err(|e| format!("bad size {v}: {e}"))
}

#[cfg(test)]
mod cli_tests {
    #[test]
    fn size_suffixes() {
        assert_eq!(super::parse_size("50000"), Ok(50000));
        assert_eq!(super::parse_size("50K"), Ok(51200));
        assert_eq!(super::parse_size("2m"), Ok(2 * 1024 * 1024));
        assert_eq!(super::parse_size(" 1 G "), Ok(1024 * 1024 * 1024));
        assert!(super::parse_size("xK").is_err());
    }
}

#[derive(Subcommand)]
pub enum ImagesAction {
    /// Download a system image (as listed by `hvolt emulator images`)
    Download {
        /// osVersion string — QUOTE IT, it contains spaces/parens:
        ///   hvolt emulator images download "HarmonyOS 6.1.1(24)"
        os_version: String,
        /// official device type (see `hvolt emulator images`)
        #[arg(long, default_value = "phone", value_parser = ["phone","tablet","2in1","2in1 Foldable","foldable","widefold","triplefold","tv","wearable","wearablekid"])]
        r#type: String,
    },
}

#[cfg(test)]
mod version_stamp_tests {
    /// `concat!` forces a literal in this file, so the guard reads the source and fails when
    /// the stamp drifts from the single source in `config.rs`.
    #[test]
    fn version_stamp_matches_the_config_constant() {
        let src = include_str!("cli.rs");
        let expected = format!(
            "\" (SDK \", \"{}\", \")\"",
            crate::config::SDK_PLATFORM_VERSION
        );
        assert!(
            src.contains(&expected),
            "cli.rs --version stamp drifted from config::SDK_PLATFORM_VERSION"
        );
    }
}
