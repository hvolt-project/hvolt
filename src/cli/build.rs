// Build: the build chain — pack, native compile, ArkTS/ABC, resources, HAP assembly.
use std::path::PathBuf;

pub(crate) fn build() {
    crate::build::deps::pipeline::build_and_sign(std::path::Path::new(".")).unwrap_or_else(|e| {
        eprintln!("✗ {}", e);
        std::process::exit(1);
    });
}

pub(crate) fn pack_hap(module_build: PathBuf, out: PathBuf, hvolt_out: bool) {
    let _ = hvolt_out;
    let inputs = crate::build::artifacts::hap_pack::inputs_from_hvolt_out(&module_build);
    match crate::build::artifacts::hap_pack::pack(&inputs, &out) {
        Ok(()) => println!("✓ packed {}", out.display()),
        Err(e) => {
            eprintln!("✗ {}", e);
            std::process::exit(1);
        }
    }
}

pub(crate) fn build_native(project: PathBuf, out: Option<PathBuf>) {
    let run = || -> Result<(), String> {
        let app = crate::support::config::AppConfig::from_file(&project.join("hvolt.toml"))
            .map_err(|e| format!("read {}: {e}", project.join("hvolt.toml").display()))?;
        let (_modules, native) = crate::translator::discover_modules(&project, &app)
            .map_err(|e| format!("discover modules: {e}"))?;
        let sdk_root =
            crate::sdk::manage::sdk_root().ok_or_else(|| "no SDK root resolved".to_string())?;
        let default_out = project.join("build/hvolt-out/libs");
        let out_dir = out.unwrap_or(default_out);
        let produced = crate::build::artifacts::native_compile::build_all(
            &sdk_root, &native, &project, &out_dir,
        )?;
        for p in &produced {
            println!("  lib {}", p.display());
        }
        println!(
            "✓ native built: {} files → {}",
            produced.len(),
            out_dir.display()
        );
        Ok(())
    };
    if let Err(e) = run() {
        eprintln!("✗ {e}");
        std::process::exit(1);
    }
}

pub(crate) fn assemble_ets(ets_dir: PathBuf, out: PathBuf, bundle: String, module: String) {
    let result = (|| -> Result<(), String> {
        let es2abc = crate::build::ets_assemble::sdk_es2abc()?;
        let kit_configs = crate::build::ets_assemble::sdk_kit_configs()?;
        let ts_root = std::env::temp_dir().join(format!("hvolt-ets-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&ts_root);
        let staged = [crate::build::ets_assemble::StageModule {
            name: &module,
            dir: &ets_dir,
            entry_file: "EntryAbility.ets",
            // The command assembles a bare ETS tree, not a configured module, so every
            // source under it is compiled.
            entries: &[],
            // A bare ETS tree assembled by this command is not a configured module, so it is
            // not a HAR.
            compile_har: false,
        }];
        let abc = crate::build::ets_assemble::assemble(
            &staged,
            &ts_root,
            &out,
            &crate::build::ets_assemble::AssembleOptions {
                bundle: &bundle,
                version: "1.0.0",
                // The command assembles a bare ETS tree, not a configured project: the profile
                // carries the values `AppFields` would derive for the `1.0.0` it names above.
                version_code: 1_000_000,
                bundle_type: "app",
                product: "default",
                target: "default",
                build_mode: "debug",
                build_version: None,
                kit_configs_dir: &kit_configs,
                es2abc: &es2abc,
                compile_context_info: None,
                packages: &[],
                target_api_version: crate::build::deps::sdk_facts::compatible_api(
                    &crate::support::config::AppConfig::from_file(
                        &std::path::Path::new(".").join("hvolt.toml"),
                    )
                    .map(|cfg| cfg.app.compile_sdk_version)
                    .unwrap_or_default(),
                )
                .unwrap_or(26),
                debug: true,
            },
        )?;
        println!("✓ assembled {}", abc.display());
        Ok(())
    })();
    if let Err(e) = result {
        eprintln!("✗ {e}");
        std::process::exit(1);
    }
}

pub(crate) fn compile_abc(ts_root: PathBuf, out: PathBuf, file_threads: u32) {
    let result = (|| -> Result<(), String> {
        let es2abc = crate::sdk::manage::sdk_root()
            .and_then(|r| {
                let sub = if cfg!(windows) {
                    "build-win"
                } else {
                    "build-mac"
                };
                crate::support::platform::exe_names("es2abc")
                    .iter()
                    .map(|n| {
                        r.join(format!(
                            "default/openharmony/ets/build-tools/ets-loader/bin/ark/{sub}/bin/{n}"
                        ))
                    })
                    .find(|p| p.exists())
            })
            .ok_or_else(|| "es2abc not found in SDK ets-loader".to_string())?;
        // The incoming tree is already staged, so a source-level `use shared`
        // directive is no longer recoverable: no staged file carries the flag.
        let shared = std::collections::BTreeSet::new();
        let merged = crate::build::compile::abc_compile::merge(
            &crate::build::compile::abc_compile::MergeInput {
                ts_root: &ts_root,
                es2abc: &es2abc,
                out_abc: &out,
                file_threads,
                target_api_version: 26,
                debug: true,
                version: "1.0.0",
                entry_record: None,
                entry_module: "",
                compile_context_info: None,
                npm: &[],
                abc_inputs: &[],
                shared: &shared,
            },
        )?;
        println!("✓ merged {}", merged.display());
        Ok(())
    })();
    if let Err(e) = result {
        eprintln!("✗ {e}");
        std::process::exit(1);
    }
}

pub(crate) fn res_compile(project: PathBuf, module: String, out: Option<PathBuf>) {
    let run = || -> Result<(), String> {
        let app = crate::support::config::AppConfig::from_file(&project.join("hvolt.toml"))
            .map_err(|e| format!("read {}: {e}", project.join("hvolt.toml").display()))?;
        let (modules, _native) = crate::translator::discover_modules(&project, &app)
            .map_err(|e| format!("discover modules: {e}"))?;
        let module_cfg = pick_module(&modules, &module)?;
        let default_out = project.join("build/hvolt-out/res");
        let out_dir = out.unwrap_or(default_out);
        let env = crate::sdk::manage::detect("")?;
        let node = crate::sdk::manage::node_bin(&env.node_home);
        let api = crate::build::deps::sdk_facts::compatible_api(&app.app.compatible_sdk_version)
            .unwrap_or_default();
        let facts = crate::build::deps::sdk_facts::SdkFacts::detect(&env.ohos_sdk, &node, api)?;
        let table = crate::build::artifacts::res_compile::run(
            &project, module_cfg, &app, &out_dir, "debug", &facts,
        )?;
        println!("✓ resources compiled: {}", table.display());
        Ok(())
    };
    if let Err(e) = run() {
        eprintln!("✗ {e}");
        std::process::exit(1);
    }
}

pub(crate) fn assemble_hap(project: PathBuf, module: String, out: Option<PathBuf>) {
    if let Err(e) = crate::build::artifacts::assemble_hap::run(&project, &module, out.as_deref()) {
        eprintln!("✗ {e}");
        std::process::exit(1);
    }
}

#[allow(clippy::too_many_arguments)] // one parameter per CLI flag, as in the clap variant
pub(crate) fn render_module_json(
    project: PathBuf,
    res_table: PathBuf,
    module: String,
    build_mode: String,
    compile_sdk_version: String,
    api_release_type: String,
    pack_info: bool,
    pkg_context: bool,
) {
    let run = || -> Result<(), String> {
        let app = crate::support::config::AppConfig::from_file(&project.join("hvolt.toml"))
            .map_err(|e| format!("read {}: {e}", project.join("hvolt.toml").display()))?;
        let (modules, native) = crate::translator::discover_modules(&project, &app)
            .map_err(|e| format!("discover modules: {e}"))?;
        let (module_dir, module_cfg) = match modules
            .iter()
            .find(|(m, _)| module.is_empty() || m == &module)
        {
            Some((m, c)) => (m.clone(), c),
            None => return Err(format!("module '{}' not found", module)),
        };
        let text = std::fs::read_to_string(&res_table)
            .map_err(|e| format!("read {}: {}", res_table.display(), e))?;
        let ids = crate::build::compile::ets_resources::parse_table(&text);
        let out = crate::build::artifacts::module_json::render(
            &crate::build::artifacts::module_json::RenderInput {
                app: &app,
                module: module_cfg,
                ids: &ids,
                build_mode: &build_mode,
                compile_sdk_version: &compile_sdk_version,
                api_release_type: &api_release_type,
                virtual_machine: "",
                has_insight_intent: false,
                has_route_map: project
                    .join(&module_dir)
                    .join("resources/base/profile/route_map.json")
                    .is_file(),
            },
        );
        let formatted =
            serde_json::to_string_pretty(&out).map_err(|e| format!("serialize: {e}"))?;
        println!("{formatted}");
        if pkg_context {
            let pc = crate::build::artifacts::pkg_context::render(
                &crate::build::artifacts::pkg_context::PkgContextInput {
                    module_name: &module_dir,
                    project_dir: &project,
                    native_deps: &native,
                    module_version: "1.0.0",
                    ets_deps: &[],
                    packages: &[],
                    app: &app,
                },
            );
            let pc_formatted = serde_json::to_string_pretty(&pc)
                .map_err(|e| format!("serialize pkgContextInfo: {e}"))?;
            println!("{}", pc_formatted);
        }
        if pack_info {
            let pi = crate::build::artifacts::pack_info::render(
                &crate::build::artifacts::pack_info::PackInput {
                    module_name: &module_dir,
                    module: module_cfg,
                    app: &app,
                    product: "default",
                    build_mode: &build_mode,
                    compile_sdk_version: &compile_sdk_version,
                    api_release_type: &api_release_type,
                    virtual_machine: "",
                    has_insight_intent: false,
                    ids: &ids,
                },
            );
            let pi_formatted = serde_json::to_string_pretty(&pi)
                .map_err(|e| format!("serialize pack.info: {e}"))?;
            println!("{}", pi_formatted);
        }
        Ok(())
    };
    if let Err(e) = run() {
        eprintln!("✗ {e}");
        std::process::exit(1);
    }
}

pub(crate) fn transpile_ets(
    input: PathBuf,
    kit_configs: Option<PathBuf>,
    bundle: Option<String>,
    module: Option<String>,
    page: Option<String>,
) {
    let result = (|| -> Result<(), String> {
        let src = std::fs::read_to_string(&input)
            .map_err(|e| format!("read {}: {}", input.display(), e))?;
        let dirs = match kit_configs {
            Some(d) => vec![d],
            None => crate::build::ets_assemble::sdk_kit_configs()?,
        };
        let route = match (&bundle, &module, &page) {
            (Some(b), Some(m), Some(p)) => Some(crate::build::lower::ets_transpile::RouteInfo {
                bundle: b,
                module: m,
                page: p,
                compile_har: false,
            }),
            _ => None,
        };
        let out = crate::build::lower::ets_transpile::transpile(
            &src,
            &crate::build::lower::ets_transpile::TranspileOptions {
                kit_config_dirs: &dirs,
                project_builders: &[],
                route,
            },
        )?;
        print!("{out}");
        Ok(())
    })();
    if let Err(e) = result {
        eprintln!("✗ {e}");
        std::process::exit(1);
    }
}

pub(crate) fn compile_ets(
    input: PathBuf,
    out: Option<PathBuf>,
    bundle: String,
    module: String,
    page: String,
    res_table: Option<PathBuf>,
) {
    // 与 `transpile-ets` 同一个入口：命令行编译不另起一套展开实现。
    let result = (|| -> Result<String, String> {
        let src = std::fs::read_to_string(&input)
            .map_err(|e| format!("read {}: {}", input.display(), e))?;
        let dirs = crate::build::ets_assemble::sdk_kit_configs()?;
        let mut js = crate::build::lower::ets_transpile::transpile(
            &src,
            &crate::build::lower::ets_transpile::TranspileOptions {
                kit_config_dirs: &dirs,
                project_builders: &[],
                route: Some(crate::build::lower::ets_transpile::RouteInfo {
                    bundle: &bundle,
                    module: &module,
                    page: &page,
                    compile_har: false,
                }),
            },
        )?;
        // `$r('app.…')` 在转译完成后再换成运行时可解析的 id：转译阶段它是普通调用，
        // 不会被子块识别成组件参数。
        if let Some(path) = &res_table {
            let text = std::fs::read_to_string(path)
                .map_err(|e| format!("read {}: {}", path.display(), e))?;
            let table = crate::build::compile::ets_resources::parse_table(&text);
            let (rewritten, missing) =
                crate::build::compile::ets_resources::rewrite(&js, &table, &bundle, &module);
            js = rewritten;
            if !missing.is_empty() {
                return Err(format!("$r 未解析: {}", missing.join(", ")));
            }
        }
        Ok(js)
    })();
    match result {
        Ok(js) => match out {
            Some(path) => match std::fs::write(&path, &js) {
                Ok(()) => println!("✓ wrote {}", path.display()),
                Err(e) => {
                    eprintln!("✗ write {}: {}", path.display(), e);
                    std::process::exit(1);
                }
            },
            None => print!("{js}"),
        },
        Err(e) => {
            eprintln!("✗ {e}");
            std::process::exit(1);
        }
    }
}

/// Pick the module to build: explicit name, else the first member.
fn pick_module<'a>(
    modules: &'a [(String, crate::support::config::ModuleConfig)],
    module: &str,
) -> Result<&'a crate::support::config::ModuleConfig, String> {
    if module.is_empty() {
        modules
            .first()
            .map(|(_, m)| m)
            .ok_or_else(|| "no modules found in hvolt.toml".to_string())
    } else {
        modules
            .iter()
            .find(|(n, _)| n == module)
            .map(|(_, m)| m)
            .ok_or_else(|| format!("module {module} not found"))
    }
}
