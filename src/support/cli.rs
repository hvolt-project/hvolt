// cli: the command-line surface (clap derive) — every subcommand and its flags.
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
        hvolt screenshot -d <serial>                      # capture from a specific device\n\nDevEco Studio is NOT required, except for `emulator` (needs its binary and images).\nSee README § DevEco Studio."
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
    /// Build: compile (own ArkTS chain), pack and sign
    Build,
    /// Sign: generate signing materials (.p12, .cer, .p7b) and configure project
    #[command(
        after_help = "`--local` works in every build. Remote signing through AppGallery Connect\n\
        is provided by a backend that is not part of this source tree; a build\n\
        configured with it (cargo feature `agc-local`) enables `hvolt sign` without\n\
        `--local` and the `hvolt agc` commands. Without it those commands explain\n\
        that the backend is missing."
    )]
    Sign {
        /// Sign this package file instead of the package the project build produced.
        /// The result stays in the package's own directory as `<name>-signed.hap`
        /// (an input ending in `-unsigned` becomes `-signed`).
        #[arg(value_name = "HAP")]
        hap: Option<PathBuf>,
        /// Signing key store (p12). Giving materials switches from the localSign test
        /// chain to these files; omit them all to sign with the test chain.
        #[arg(long, value_name = "FILE")]
        p12: Option<PathBuf>,
        /// Provisioning profile (p7b).
        #[arg(long, value_name = "FILE")]
        profile: Option<PathBuf>,
        /// Signing certificate chain (pem).
        #[arg(long, value_name = "FILE")]
        cert: Option<PathBuf>,
        /// Key alias inside the p12.
        #[arg(long, value_name = "ALIAS", default_value = "hvolt")]
        key_alias: String,
        /// Password of the p12.
        #[arg(long, value_name = "PASSWORD", default_value = "")]
        p12_password: String,
        /// Password of the key entry.
        #[arg(long, value_name = "PASSWORD", default_value = "")]
        key_password: String,
        /// bundleName to bind the profile to when the package does not declare one.
        #[arg(long, value_name = "BUNDLE")]
        bundle: Option<String>,
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
    /// Push signed.hap to device (default: newest output/*-signed.hap)
    Push {
        /// Device address (e.g. 192.168.1.100:38565 or serial number)
        device: Option<String>,
        /// Target device (same as positional; for flag-style muscle memory)
        #[arg(short = 'd', long = "device", value_name = "DEVICE")]
        device_flag: Option<String>,
        /// Install this package file instead of output/*-signed.hap
        #[arg(long, value_name = "FILE")]
        hap: Option<PathBuf>,
    },
    /// Pack a HAP from an `assemble-hap` output directory (experimental)
    #[command(hide = true)]
    PackHap {
        /// Module build directory (…/build/default), or an hvolt-out directory with --hvolt-out
        module_build: std::path::PathBuf,
        /// Output .hap path
        out: std::path::PathBuf,
        /// Interpret the first positional as an `hvolt assemble-hap` output dir
        #[arg(long)]
        hvolt_out: bool,
    },
    /// Merge expanded .ts files into modules.abc with the SDK es2abc (experimental)
    ///
    /// Writes filesInfo.txt/npmEntries.txt and runs
    /// `es2abc --merge-abc "@filesInfo" --npm-module-entry-list npmEntries
    /// --output modules.abc --file-threads N`.
    #[command(hide = true)]
    /// Compile an .ets source tree to a merged modules.abc: every page goes
    /// through the transpile pass, every other file (EntryAbility etc.) gets
    /// its kit imports rewritten, then es2abc merges the tree.
    AssembleEts {
        /// Directory holding the .ets sources (module root, e.g. `src/`)
        #[arg(value_name = "ETS_DIR")]
        ets_dir: std::path::PathBuf,
        /// Output modules.abc path
        #[arg(long, value_name = "FILE")]
        out: std::path::PathBuf,
        /// bundleName recorded in @Entry page registration
        #[arg(long, default_value = "")]
        bundle: String,
        /// moduleName recorded in @Entry page registration
        #[arg(long, default_value = "")]
        module: String,
    },
    CompileAbc {
        /// Directory holding the pre-pass expanded .ts files
        ts_root: std::path::PathBuf,
        /// Output modules.abc path
        out: std::path::PathBuf,
        /// Worker threads (default: 8)
        #[arg(long, default_value_t = 8)]
        file_threads: u32,
    },
    /// Build native modules with the SDK cmake+ninja (experimental)
    ///
    /// For every `libs/<name>` with a CMakeLists.txt: configure with
    /// ohos.toolchain.cmake (arm64-v8a), run ninja, llvm-strip the .so files,
    /// and copy them into `<out>/arm64-v8a/`.
    #[command(hide = true)]
    BuildNative {
        /// Project root (contains hvolt.toml)
        project: std::path::PathBuf,
        /// Output directory for `arm64-v8a/*.so` (default: <project>/build/hvolt-out/libs)
        #[arg(long, value_name = "DIR")]
        out: Option<std::path::PathBuf>,
    },
    /// Compile the module's resources with the SDK restool (experimental)
    ///
    /// Stages a resource tree (project resources/ + generated profile/main_pages.json),
    /// builds the resConfig.json restool consumes, and produces resources.index,
    /// resources/ and ResourceTable.txt (+ .h/.ts) under <project>/build/hvolt-out/res.
    #[command(hide = true)]
    ResCompile {
        /// Project root (contains hvolt.toml)
        project: std::path::PathBuf,
        /// Module name inside the project (default: first member)
        #[arg(long, default_value = "")]
        module: String,
        /// Output directory (default: <project>/build/hvolt-out/res)
        #[arg(long, value_name = "DIR")]
        out: Option<std::path::PathBuf>,
    },
    /// Assemble the hvolt HAP parts: resources + module.json + pack.info + pkgContextInfo
    ///
    /// Runs the res-compile pipeline, then renders module.json, pack.info and
    /// pkgContextInfo.json from the same manifest. Outputs land in
    /// <project>/build/hvolt-out/ (module.json, resources.index, resources/,
    /// pack.info, pkgContextInfo.json). modules.abc and libs/ come from the
    /// compile and native tasks, which plug into the same directory later.
    #[command(hide = true)]
    AssembleHap {
        /// Project root (contains hvolt.toml)
        project: std::path::PathBuf,
        /// Module name inside the project (default: first member)
        #[arg(long, default_value = "")]
        module: String,
        /// Output directory (default: <project>/build/hvolt-out)
        #[arg(long, value_name = "DIR")]
        out: Option<std::path::PathBuf>,
    },
    /// Render module.json from hvolt.toml + a restool ResourceTable (experimental)
    #[command(hide = true)]
    RenderModuleJson {
        /// Project root (contains hvolt.toml)
        project: std::path::PathBuf,
        /// restool ResourceTable.txt
        #[arg(long, value_name = "FILE")]
        res_table: std::path::PathBuf,
        /// Module name inside the project (default: first member)
        #[arg(long, default_value = "")]
        module: String,
        /// Build mode: debug or release
        #[arg(long, default_value = "release")]
        build_mode: String,
        /// Also print pack.info derived from the same manifest
        #[arg(long)]
        pack_info: bool,
        /// Also print pkgContextInfo.json derived from dependencies
        #[arg(long)]
        pkg_context: bool,
        /// Full SDK version, e.g. 26.0.0.23
        #[arg(long, default_value = "26.0.0.23")]
        compile_sdk_version: String,
        /// SDK release type
        #[arg(long, default_value = "Beta1")]
        api_release_type: String,
    },
    /// Expand an ArkTS source to the .ts es2abc consumes (experimental)
    ///
    /// Rewrites `@kit.*` imports into `@ohos:*` requests (kit_configs from the
    /// SDK), normalizes `.so` imports, and re-indents to a stable layout
    /// shape. TDD slice T1/T2; struct expansion lands with T3.
    #[command(hide = true)]
    TranspileEts {
        /// ArkTS source file (.ets)
        input: std::path::PathBuf,
        /// SDK kit_configs directory (default: from the SDK root)
        #[arg(long, value_name = "DIR")]
        kit_configs: Option<std::path::PathBuf>,
        /// Bundle name for the `registerNamedRoute` footer of @Entry pages
        #[arg(long, value_name = "NAME")]
        bundle: Option<String>,
        /// Module name for the `registerNamedRoute` footer
        #[arg(long, value_name = "NAME")]
        module: Option<String>,
        /// Page path relative to `src/main/ets` (e.g. `pages/Index`)
        #[arg(long, value_name = "PATH")]
        page: Option<String>,
    },
    /// Compile ArkTS sources to the JS that es2abc consumes (experimental)
    ///
    /// Reads a component source, rewrites the ArkTS-only constructs (`struct`, builder
    /// blocks), and prints the `ViewPU` subclass the ArkUI runtime expects.
    #[command(hide = true)]
    CompileEts {
        /// ArkTS source file (.ets)
        input: std::path::PathBuf,
        /// Write the generated JS here; stdout when omitted
        #[arg(long, value_name = "FILE")]
        out: Option<std::path::PathBuf>,
        /// bundleName recorded in the page registration
        #[arg(long, default_value = "")]
        bundle: String,
        /// moduleName recorded in the page registration
        #[arg(long, default_value = "")]
        module: String,
        /// pagePath recorded in the page registration
        #[arg(long, default_value = "")]
        page: String,
        /// ResourceTable.txt from restool; enables `$r('app.<kind>.<name>')` resolution
        #[arg(long, value_name = "FILE")]
        res_table: Option<std::path::PathBuf>,
    },
    /// Execute a shell command, copy a file, or install a package on a device (experimental)
    ///
    /// Speaks the device protocol directly through hvolt's own transport (`src/device/usb.rs`),
    /// with no external tool: `ADDR` is `<host>:<port>` over TCP or a USB serial number;
    /// `--shell` runs one command, `--put` copies a file, `--install` installs a package; with
    /// no flag the device's auth state is reported. Hidden until the protocol work is complete.
    #[command(hide = true)]
    Exec {
        /// `<host>:<port>` of the device daemon, e.g. 192.168.1.3:8710
        addr: String,
        /// Run one shell command over the authenticated connection and print its output
        #[arg(long, value_name = "COMMAND")]
        shell: Option<String>,
        /// Copy a local file to the device: `--put <local> <remote>`
        #[arg(long, num_args = 2, value_names = ["LOCAL", "REMOTE"])]
        put: Option<Vec<String>>,
        /// Install a package on the device
        #[arg(long, value_name = "HAP")]
        install: Option<std::path::PathBuf>,
    },
    /// Build + sign + push + launch in one shot
    Run {
        /// Device address (e.g. 192.168.1.100:38565 or serial number)
        device: Option<String>,
        /// Target device (same as positional)
        #[arg(short = 'd', long = "device", value_name = "DEVICE")]
        device_flag: Option<String>,
    },
    /// Package-side quality gate: duplicate files, oversized files, per-suffix totals
    Check {
        /// also flag files over this size (bytes, or 50K / 2M / 1G, case-insensitive)
        #[arg(long, value_parser = parse_size)]
        size_limit: Option<u64>,
    },
    /// Parse every .ets under a directory and report which ones the component parser rejects
    ParseReport {
        /// directory to scan (default: current)
        #[arg(value_name = "DIR")]
        dir: Option<String>,
    },
    /// Report import names a staged module does not export (export/link mismatches)
    #[command(hide = true)]
    ExportReport {
        /// staged tree to scan (the `HVOLT_KEEP_TS` output of a build)
        ts_root: std::path::PathBuf,
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
    /// Register a device by UDID
    Device {
        #[command(subcommand)]
        action: AgcDeviceAction,
    },
    /// Show current login user info
    Whoami,
}

/// `hvolt agc device` subcommands.
#[derive(Subcommand)]
pub enum AgcDeviceAction {
    /// Register a device UDID (no-op if already registered)
    Add {
        /// Device UDID (hdc shell bm get --udid)
        udid: String,
        /// Display name in AGC (default: hvolt-device-<last 6 of udid>)
        #[arg(long)]
        name: Option<String>,
    },
    /// Delete a device record by its AGC id (see `hvolt agc devices`)
    Delete {
        /// Service-side device id, not the UDID
        id: String,
    },
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
        /// Device serial on that host
        serial: String,
        /// Absolute path of hdc on the device host (legacy cross-machine path)
        #[arg(long)]
        hdc: Option<String>,
    },
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
            crate::support::config::SDK_PLATFORM_VERSION
        );
        assert!(
            src.contains(&expected),
            "cli.rs --version stamp drifted from config::SDK_PLATFORM_VERSION"
        );
    }
}
