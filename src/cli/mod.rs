// cli: command dispatch (one arm per clap variant) and one module per command family.
//
//   build.rs         build chain stages: pack, native, ArkTS/ABC, resources, HAP assembly
//   sign.rs          package signing with given materials or the localSign test chain
//   deploy.rs        push, run, uninstall and HAP introspection after a build
//   device.rs        hdc wire probe, emulator, registry, screenshot, UI dump, logs, files
//   project.rs       init, import, doctor, clean, check, reports, parse checks, language server
//   sdk.rs           `hvolt sdk` subcommands
//   agc.rs           `hvolt agc` subcommands
//   config_skill.rs  `hvolt config` values and the embedded-document commands

pub(crate) mod agc;
pub(crate) mod build;
pub(crate) mod config_skill;
pub(crate) mod deploy;
pub(crate) mod device;
pub(crate) mod project;
pub(crate) mod sdk;
pub(crate) mod sign;

use crate::support::cli::Command;

/// Hand one parsed subcommand to its family module; each handler owns its exit codes.
pub(crate) fn dispatch(command: Command) {
    match command {
        Command::Init { name } => project::init(name),
        Command::Import { from, out, dry_run } => project::import(from, out, dry_run),
        Command::Build => build::build(),
        Command::Sign {
            hap,
            local,
            apl,
            flavor,
            p12,
            profile,
            cert,
            key_alias,
            p12_password,
            key_password,
            bundle,
        } => sign::sign(
            hap,
            local,
            apl,
            flavor,
            p12,
            profile,
            cert,
            key_alias,
            p12_password,
            key_password,
            bundle,
        ),
        Command::Push {
            device,
            device_flag,
            hap,
        } => deploy::push(device, device_flag, hap),
        Command::PackHap {
            module_build,
            out,
            hvolt_out,
        } => build::pack_hap(module_build, out, hvolt_out),
        Command::BuildNative { project, out } => build::build_native(project, out),
        Command::AssembleEts {
            ets_dir,
            out,
            bundle,
            module,
        } => build::assemble_ets(ets_dir, out, bundle, module),
        Command::CompileAbc {
            ts_root,
            out,
            file_threads,
        } => build::compile_abc(ts_root, out, file_threads),
        Command::ResCompile {
            project,
            module,
            out,
        } => build::res_compile(project, module, out),
        Command::AssembleHap {
            project,
            module,
            out,
        } => build::assemble_hap(project, module, out),
        Command::RenderModuleJson {
            project,
            res_table,
            module,
            build_mode,
            compile_sdk_version,
            api_release_type,
            pack_info,
            pkg_context,
        } => build::render_module_json(
            project,
            res_table,
            module,
            build_mode,
            compile_sdk_version,
            api_release_type,
            pack_info,
            pkg_context,
        ),
        Command::TranspileEts {
            input,
            kit_configs,
            bundle,
            module,
            page,
        } => build::transpile_ets(input, kit_configs, bundle, module, page),
        Command::CompileEts {
            input,
            out,
            bundle,
            module,
            page,
            res_table,
        } => build::compile_ets(input, out, bundle, module, page, res_table),
        Command::Exec {
            addr,
            shell,
            put,
            install,
        } => device::exec(addr, shell, put, install),
        Command::Run {
            device,
            device_flag,
        } => deploy::run(device, device_flag),
        Command::Emulator { action } => device::emulator(action),
        Command::Device { action } => device::device(action),
        Command::Screenshot { out, device } => device::screenshot(out, device),
        Command::Ui { action } => device::ui(action),
        Command::Log {
            level,
            bundle,
            tag,
            domain,
            keyword,
            crash,
            follow,
            tail,
            device,
        } => device::log(
            level, bundle, tag, domain, keyword, crash, follow, tail, device,
        ),
        Command::File { action } => device::file(action),
        Command::Uninstall { bundle, device } => deploy::uninstall(bundle, device),
        Command::Doctor => project::doctor(),
        Command::ExportReport { ts_root } => project::export_report(ts_root),
        Command::ParseReport { dir } => project::parse_report(dir),
        Command::Check { size_limit } => project::check(size_limit),
        Command::Lsp { stdio, sdk } => project::lsp(stdio, sdk),
        Command::Hap { sub } => deploy::hap(sub),
        Command::Clean => project::clean(),
        Command::Sdk { action } => sdk::sdk(action),
        Command::Agc { action } => agc::agc(action),
        Command::Config { action } => config_skill::config(action),
        Command::Skill { action } => config_skill::skill(action),
    }
}
