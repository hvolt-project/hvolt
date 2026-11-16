// pipeline: build + sign + push + launch workflow.

use std::path::Path;

use crate::sdk::manage::SdkEnv;
use crate::support::config::{AppConfig, ModuleConfig};

/// Module-relative sources the reference compiles the entry module from: every page route,
/// every ability and the module's own entry. `loader.json`'s `compileEntry` (pages and
/// router-map targets) and module.json's ability `srcEntry` are the reference's counterparts
/// (`generate-loader-json.js`, `main.js`: `loadEntryObj`).
fn compile_roots(cfg: Option<&ModuleConfig>, dir: &Path) -> Result<Vec<String>, String> {
    let Some(cfg) = cfg else {
        return Ok(Vec::new());
    };
    let module = &cfg.module;
    let mut roots: Vec<String> = module.routes.clone();
    roots.push(module.entry.clone());
    for ability in &module.abilities {
        if ability.entry.is_empty() {
            continue;
        }
        let dir = ability.dir.trim_matches('/');
        roots.push(if dir.is_empty() {
            ability.entry.clone()
        } else {
            format!("{dir}/{}", ability.entry)
        });
    }
    if let Some(src_entry) = &module.src_entry {
        roots.push(src_entry.trim_start_matches("./").to_string());
    }
    roots.extend(
        crate::build::deps::routes::route_entries(Some(module), dir)?
            .into_iter()
            .map(|route| route.page),
    );
    roots.sort();
    roots.dedup();
    Ok(roots)
}

/// One `routerMap` entry, keyed the way the lowering pass looks a file up.
///
/// `main.js` (`:10619-10660`) builds `storedFileInfo.routerInfo: Map<absolute file,
/// [{name, buildFunction}]>` from the module's `routerMap`, keeping an entry only when
/// `item.pageSourceFile && (item.name || item.name === '') && item.buildFunction`. The reference
/// keys that map by absolute path; this build keys it by the source coordinate it records for a
/// file (`<module>/src/main/ets/<page>.ets`), so `pageSourceFile` — which `route_map.json`
/// writes module-relative (`src/main/ets/…`) — is prefixed with the module's directory name.
#[derive(Clone, Debug)]
pub(crate) struct RouterMapEntry {
    pub(crate) name: String,
    pub(crate) build_function: String,
    pub(crate) page_source_file: String,
}

/// Read one module's `routerMap`, publish it for the lowering pass, and return it.
///
/// `route_entries` reads the same profile for the compile roots; this reads the rest of each
/// entry (`name`, `buildFunction`) — the pair `createNavigationInit` needs to register a global
/// `@Builder` function under the name the router map gave it.
pub(crate) fn router_map(
    module: Option<&crate::support::config::ModuleInfo>,
    module_dir: &Path,
) -> Result<Vec<RouterMapEntry>, String> {
    let entries: Vec<RouterMapEntry> =
        crate::build::deps::routes::route_entries(module, module_dir)?
            .into_iter()
            .map(|route| RouterMapEntry {
                name: route.name,
                build_function: route.build_function,
                page_source_file: format!(
                    "{}/src/main/ets/{}",
                    module
                        .map(|module| module.name.as_str())
                        .unwrap_or_default(),
                    route.page.replace('\\', "/")
                ),
            })
            .collect();
    crate::build::stage::process_ui_syntax::set_router_map(&entries);
    Ok(entries)
}

/// Native (standalone, no-ohpm) build chain:
/// build-native → assemble-hap (resources) → assemble-ets (abc) → pack-hap.
///
/// The translated DevEco layout (assemble) is NOT produced: everything stays
/// under `<project>/build/hvolt-out`, and the HAP is packed from there.
/// Returns the failure instead of exiting: a library-shaped pipeline stays
/// composable and its error paths stay assertable.
/// Copy the precompiled libraries each module ships beside itself.
///
/// The packer reads one flat `libs/<abi>/`, and a module's own `libs/<abi>/*.so` is the
/// prebuilt half of its native surface (`libflclash.so`, 63 MB, is built outside this tree).
/// Nothing copied them, so the device reported
/// `export objects of native so is undefined, so name is @normalized:Y&&&libflclash.so&`
/// and the proxy core was never loaded (clashbox, measured 2026-11-08).
fn collect_prebuilt_libs(
    project_dir: &Path,
    module_dir: &str,
    out_dir: &Path,
) -> Result<Vec<std::path::PathBuf>, String> {
    let libs = out_dir.join("arm64-v8a");
    std::fs::create_dir_all(&libs).map_err(|e| format!("mkdir {}: {e}", libs.display()))?;
    let mut copied = Vec::new();
    let source = project_dir.join(module_dir).join("libs").join("arm64-v8a");
    if let Ok(entries) = std::fs::read_dir(&source) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_none_or(|ext| ext != "so") {
                continue;
            }
            let Some(file_name) = path.file_name() else {
                continue;
            };
            let dest = libs.join(file_name);
            std::fs::copy(&path, &dest).map_err(|e| format!("copy {}: {e}", path.display()))?;
            copied.push(dest);
        }
    }
    Ok(copied)
}

/// Build the native half of every module that ships C++ sources.
///
/// The flat layout keeps them at `<module>/cpp/CMakeLists.txt`, while `build_all` looks only
/// under `libs/<name>/` — so a flat project produced no `.so` at all, and the device reported
/// `export objects of native so is undefined` for `libproxy_core.so` and `libxb_components.so`
/// (clashbox, measured 2026-11-08).
fn build_module_native(
    sdk_root: &Path,
    project_dir: &Path,
    name: &str,
    out_dir: &Path,
) -> Result<Vec<std::path::PathBuf>, String> {
    let libs = out_dir.join("arm64-v8a");
    std::fs::create_dir_all(&libs).map_err(|e| format!("mkdir {}: {e}", libs.display()))?;
    let mut produced = Vec::new();
    let module_dir = std::path::absolute(project_dir.join(name).join("cpp"))
        .map_err(|e| format!("absolute {name}: {e}"))?;
    if module_dir.join("CMakeLists.txt").exists() {
        let build_dir = out_dir
            .parent()
            .unwrap_or(out_dir)
            .join(format!(".native-build-{name}"));
        let built = crate::build::artifacts::native_compile::build(
            sdk_root,
            &module_dir,
            &build_dir,
            "Debug",
        )?;
        if let Err(e) = crate::build::artifacts::native_compile::strip(sdk_root, &built) {
            println!("  (strip skipped: {e})");
        }
        for entry in std::fs::read_dir(&built).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.extension().is_none_or(|ext| ext != "so") {
                continue;
            }
            let Some(file_name) = path.file_name() else {
                continue;
            };
            let dest = libs.join(file_name);
            std::fs::copy(&path, &dest).map_err(|e| format!("copy {}: {e}", path.display()))?;
            produced.push(dest);
        }
    }
    Ok(produced)
}

/// One package this build produces: an entry/feature module's `.hap` or a shared module's
/// `.hsp`, together with the output directory and the staged module trees its abc carries.
///
/// hvigor builds each module as its own package: `assembleHap` for an entry/feature module and
/// `assembleHsp` for a `type = "shared"` one, each with its own `module.json`, `pack.info`,
/// `pkgContextInfo.json`, `resources.index` and `ets/modules.abc`
/// (`.scratch/multi-module/truth/`, `truth/modules.json`).
struct PlannedPackage {
    /// The module's directory under the project (the key `assemble_hap` looks it up by).
    dir: String,
    /// The module's declared name, which the package and its records carry.
    name: String,
    /// A shared package: `.hsp`, compiled into its own abc only.
    is_hsp: bool,
    out_dir: std::path::PathBuf,
    /// The staged module trees whose sources go into this package's abc.
    abc_modules: Vec<String>,
}

impl PlannedPackage {
    /// The artifact's extension: a shared package is a `.hsp`, everything else a `.hap`.
    fn extension(&self) -> &'static str {
        if self.is_hsp {
            "hsp"
        } else {
            "hap"
        }
    }
}

fn native_build(project_dir: &Path, product: Option<&str>) -> Result<(AppConfig, SdkEnv), String> {
    let root_toml = project_dir.join("hvolt.toml");
    // `from_file` returns the file as written; the product's overrides are resolved here, once,
    // so every stage below reads one effective config.
    let declared = AppConfig::from_file(&root_toml)?;
    let product_name = product
        .map(str::to_string)
        .unwrap_or_else(|| declared.default_product_name().to_string());
    let app_config = declared.with_product(&product_name)?;
    println!("  product: {product_name}");

    let sdk_env = crate::sdk::manage::detect(app_config.app.sdk_path.as_deref().unwrap_or(""))
        .map_err(|e| format!("SDK not available: {}", e))?;
    println!("  SDK detected: {}", sdk_env.ohos_sdk.display());

    let (modules, native) = crate::translator::discover_modules(project_dir, &app_config)
        .map_err(|e| format!("discover modules: {e}"))?;
    let out_root = project_dir.join("build/hvolt-out");
    let sdk_root =
        crate::sdk::manage::sdk_root().ok_or_else(|| "no SDK root resolved".to_string())?;

    // 1) one package per module type.
    //
    // A shared module (HSP) is its own package; every other member is a HAP. A HAR member
    // produces no artifact in M1: its sources ride in the first package's abc, which is what
    // the single merged abc did before.
    let hsp_names: Vec<String> = modules
        .iter()
        .filter(|(_, cfg)| cfg.module.is_hsp())
        .map(|(_, cfg)| cfg.module.name.clone())
        .collect();
    // The one criterion for the single-package vs multi-package behaviour in this build: a
    // project that declares a shared member produces more than one package. `rpcid.sc` and the
    // unconditional C++ runtime belong to the legacy single-package package content (pinned by
    // `baseline/hello-world-before.txt`); the reference packages of a multi-package build carry
    // neither.
    let multi_package = !hsp_names.is_empty();
    let har_names: Vec<String> = modules
        .iter()
        .filter(|(_, cfg)| cfg.module.module_type == "har")
        .map(|(_, cfg)| cfg.module.name.clone())
        .collect();
    let mut planned: Vec<PlannedPackage> = Vec::new();
    for (dir, cfg) in &modules {
        if cfg.module.module_type == "har" {
            continue;
        }
        let primary = planned.is_empty();
        let mut abc_modules = vec![cfg.module.name.clone()];
        if primary {
            abc_modules.extend(har_names.iter().cloned());
        }
        planned.push(PlannedPackage {
            dir: dir.clone(),
            name: cfg.module.name.clone(),
            is_hsp: cfg.module.is_hsp(),
            // The first package keeps `build/hvolt-out` itself: it is the directory every other
            // tool reads (`assemble-hap`, `pack-hap`, the oracle's artifact harvest), so the
            // single-module layout does not move.
            out_dir: if primary {
                out_root.clone()
            } else {
                out_root.join(&cfg.module.name)
            },
            abc_modules,
        });
    }
    if planned.is_empty() {
        return Err(
            "no package-producing module: every member is a HAR, and M1 does not produce one"
                .to_string(),
        );
    }
    // The legacy single-package path keeps the package content hvolt 26.0.0-9 emitted
    // (`libs/arm64-v8a/libc++_shared.so` and `rpcid.sc`), which the pinned hello-world baseline
    // records. A project with more than one package follows the reference instead.
    let single_package = planned.len() == 1;

    // A native module lives where its `[dependencies]` entry says (`path = "mylib"`), not at
    // a fixed `libs/<name>`; `hvolt import` writes the former.
    let native_names: Vec<(String, std::path::PathBuf)> = native
        .iter()
        .map(|(name, _)| {
            let dir = app_config
                .dependencies
                .get(name)
                .and_then(|dep| dep.path.as_deref())
                .map(|path| project_dir.join(path))
                .unwrap_or_else(|| project_dir.join("libs").join(name));
            (name.clone(), dir)
        })
        .collect();
    let primary_out = planned[0].out_dir.clone();
    let produced = crate::build::artifacts::native_compile::build_all(
        &sdk_root,
        &native_names,
        &primary_out.join("libs"),
    )?;
    if !produced.is_empty() {
        println!("✓ native lib(s) from libs/: {}", produced.len());
    }
    // The `.so` files this build actually produced, per package. The packer is fed this list
    // rather than the directory: the directory is reused between builds, and a stale library
    // must not reappear in a package that no longer has native code.
    let mut libs_by_out: std::collections::BTreeMap<std::path::PathBuf, Vec<std::path::PathBuf>> =
        std::collections::BTreeMap::new();
    libs_by_out.insert(primary_out.clone(), produced.clone());
    // 2) native libs, per package: a package carries only the `.so` files its own module ships,
    //    so a project without native code produces no `libs/` at all
    //    (`.scratch/multi-module/truth/entry/contents.txt` has none, while hvolt 26.0.0-9
    //    injected `libs/arm64-v8a/libc++_shared.so` into every package).
    for (index, package) in planned.iter().enumerate() {
        let libs_dir = package.out_dir.join("libs");
        let built = build_module_native(&sdk_root, project_dir, &package.dir, &libs_dir)?;
        let prebuilt = collect_prebuilt_libs(project_dir, &package.dir, &libs_dir)?;
        if !built.is_empty() {
            println!("✓ native lib(s) from <module>/cpp: {}", built.len());
        }
        if !prebuilt.is_empty() {
            println!("✓ prebuilt lib(s) copied: {}", prebuilt.len());
            // A prebuilt library ships whatever the project happened to build it with —
            // `libmush.so` arrives at 851,792 B with `debug_info`, while the package the
            // legacy chain produced carried 231,280 B. The package gets the stripped form.
            let mut dirs: std::collections::BTreeSet<std::path::PathBuf> = Default::default();
            for file in &prebuilt {
                if let Some(parent) = file.parent() {
                    dirs.insert(parent.to_path_buf());
                }
            }
            for dir in dirs {
                if let Err(e) = crate::build::artifacts::native_compile::strip(&sdk_root, &dir) {
                    println!("  (strip skipped: {e})");
                }
            }
        }
        // The C++ runtime every native module links against. Without it a C++ module's exports
        // read `undefined` at import time (see `cxx_runtime`). A package with no native code of
        // its own gets none in a multi-package build: the reference packages of this project
        // carry no `libs/`. The legacy single-package build keeps emitting it — the pinned
        // hello-world baseline carries `libs/arm64-v8a/libc++_shared.so` even without native
        // sources.
        let has_native = single_package
            || !built.is_empty()
            || !prebuilt.is_empty()
            || (index == 0 && !produced.is_empty());
        let mut files = libs_by_out.remove(&package.out_dir).unwrap_or_default();
        files.extend(built);
        files.extend(prebuilt);
        if has_native {
            if let Some(runtime) = crate::build::artifacts::native_compile::cxx_runtime(&sdk_root) {
                let abi_dir = libs_dir.join("arm64-v8a");
                std::fs::create_dir_all(&abi_dir)
                    .map_err(|e| format!("mkdir {}: {e}", abi_dir.display()))?;
                let dest = abi_dir.join("libc++_shared.so");
                std::fs::copy(&runtime, &dest)
                    .map_err(|e| format!("copy {}: {e}", runtime.display()))?;
                files.push(dest);
            }
        }
        files.sort();
        files.dedup();
        libs_by_out.insert(package.out_dir.clone(), files);
    }

    // 3) resources + module metadata, one output directory per package
    for package in &planned {
        crate::build::artifacts::assemble_hap::run_with(
            project_dir,
            &package.dir,
            Some(&package.out_dir),
            &app_config,
        )?;
    }

    // 3.5) pre-compile steps: they may generate a source file, so they have to run before the
    // ArkTS pass reads the tree. A generation step placed after the compile would be invisible
    // to that build.
    if !app_config.app.build.steps.is_empty() {
        let primary = planned
            .first()
            .map(|package| package.out_dir.clone())
            .unwrap_or_else(|| out_root.clone());
        let module_name = planned
            .first()
            .map(|package| package.name.clone())
            .unwrap_or_else(|| "entry".to_string());
        let ctx = crate::build::steps::StepContext {
            project_dir,
            out_dir: &primary,
            module: &module_name,
            variant: &product_name,
        };
        crate::build::steps::run_phase(&ctx, &app_config.app.build.steps, "pre")
            .map_err(|e| format!("build step failed: {}", e))?;
    }

    // 4) modules.abc from each module's .ets tree
    // Record names carry the module's logical name, not its directory: the device
    // resolves `<module>/src/main/ets/<srcEntry path>` and a directory-derived
    // name (`src`) never matches.
    /// One staged module: name, dir, entry file, compile roots, is-HAR, declared `main`.
    type Staged = (
        String,
        std::path::PathBuf,
        String,
        Vec<String>,
        bool,
        Option<String>,
    );
    // Every ETS module is staged for the one transpile pass. A shared package's whole tree is
    // compiled (the reference puts an HSP's sources into `compileEntry`,
    // `generate-loader-json.js`: `allowToCollectEtsFolder`); an entry/feature module compiles
    // the sources the reference hands to rollup — every page route, every ability and the
    // module's own entry, plus whatever those import.
    let mut staged: Vec<Staged> = Vec::new();
    for (dir, cfg) in &modules {
        let module_dir = project_dir.join(dir);
        let roots = if cfg.module.is_hsp() || cfg.module.module_type == "har" {
            Vec::new()
        } else {
            compile_roots(Some(cfg), &module_dir)?
        };
        staged.push((
            cfg.module.name.clone(),
            module_dir,
            cfg.module.entry.clone(),
            roots,
            cfg.module.module_type == "har",
            cfg.module.main.clone(),
        ));
    }
    // The router map is a per-module profile; publish each one before any file is lowered, so a
    // file named by it can register its builders (`createNavigationInit`).
    //
    // The key prefix is the module's declared name (`hvolt.toml`'s `module.name`), not the
    // directory the module was staged under: the lookup side builds its key from the recorded
    // source coordinate (`<module>/src/main/ets/<page>.ets`), and a staged directory can be
    // called `src`, which never matches it.
    for (_name, dir, _, _, _, _) in &staged {
        // The table comes from the module's config when it declares one and from its
        // `route_map.json` otherwise. The config is read by directory, not by declared name:
        // a staged directory can be called `src` while the module is `entry`.
        // A module without its own `hvolt.toml` still contributes whatever its
        // `route_map.json` declares; a broken one is reported rather than skipped silently.
        let cfg = crate::build::deps::routes::module_config(dir)?;
        router_map(Some(&cfg.module), dir)?;
    }
    // A shared package's records and import urls live at `src/main/` rather than
    // `src/main/ets/`, and a request for one is a normalized url, not a package record
    // (`.scratch/multi-module/truth/{shared,entry}/modules.disasm`).
    let shared_modules: std::collections::BTreeMap<String, String> = modules
        .iter()
        .filter(|(_, cfg)| cfg.module.is_hsp())
        .map(|(_, cfg)| (cfg.module.name.clone(), cfg.module.shared_entry_path()))
        .collect();
    crate::build::compile::abc_compile::set_shared_modules(shared_modules);
    // `apiAvailable` is left untouched on a point version
    // (`process_available_statement.js::isPointVersion(originCompatibleSdkVersion)`), so the
    // transpiler needs the project's compatible SDK version before any file is lowered.
    crate::build::ets::api_available::set_compatible_version(
        &app_config.app.compatible_sdk_version,
    );
    let stage_refs: Vec<crate::build::ets_assemble::StageModule<'_>> = staged
        .iter()
        .map(|(name, dir, entry, entries, compile_har, main)| {
            crate::build::ets_assemble::StageModule {
                name,
                dir,
                entry_file: entry,
                entries,
                compile_har: *compile_har,
                main: main.as_deref(),
            }
        })
        .collect();
    // `--compile-context-info` is the record-name form the reference emits, not the raw table:
    // `{ "hspPkgNames": [], "compileEntries": ["&<pkg>/<rel>&", …], "updateVersionInfo": {} }`.
    // The list names the modules this package's abc carries, so it is per package
    // (`.scratch/multi-module/reference-answers.md` §1.3:
    // `&box64hsp/build/generated/r/ResourceTable&`, `&box64hsp/src/main/Index&`).
    // `hspPkgNames` is the shared-package list; both reference contexts carry `[]`, and no
    // non-empty sample was obtained, so it stays empty.
    fn add_compile_entry(entries: &mut Vec<String>, module: &str, rel: &str) {
        let record = format!(
            "&{}&",
            crate::build::compile::abc_compile::module_path(module, rel)
        );
        if !entries.contains(&record) {
            entries.push(record);
        }
    }
    // The compile roots each module was staged from; a module with no declared roots (HSP/HAR)
    // is compiled from its whole tree and is named by its own entry file.
    let staged_roots: std::collections::BTreeMap<String, Vec<String>> = staged
        .iter()
        .map(|(name, _, entry, roots, _, _)| {
            let mut roots = roots.clone();
            if roots.is_empty() {
                roots.push(entry.clone());
            }
            (name.clone(), roots)
        })
        .collect();
    let mut compile_contexts: Vec<std::path::PathBuf> = Vec::new();
    for package in &planned {
        let context = package.out_dir.join("compileContextInfo.json");
        let mut compile_entries: Vec<String> = Vec::new();
        // The generated resource table is a compiled module of every package, and the reference
        // context names it first.
        add_compile_entry(
            &mut compile_entries,
            &package.name,
            "build/generated/r/ResourceTable",
        );
        for module in &package.abc_modules {
            if let Some(roots) = staged_roots.get(module) {
                for rel in roots {
                    let rel = rel.trim_start_matches("./").trim_end_matches(".ets");
                    add_compile_entry(&mut compile_entries, module, rel);
                }
            }
        }
        let wrapped = serde_json::json!({
            "hspPkgNames": [],
            "compileEntries": compile_entries,
            "updateVersionInfo": {},
        });
        // The path is handed to es2abc below; a failed write there would compile against a
        // missing context file, so the failure has to surface here.
        std::fs::write(
            &context,
            serde_json::to_string(&wrapped).map_err(|e| e.to_string())?,
        )
        .map_err(|e| format!("write {}: {e}", context.display()))?;
        compile_contexts.push(context);
    }
    // Third-party packages the app declares; their records are part of the same abc.
    // 编译前核对每个具名 import 在源模块里确有对应导出。设备端只会在启动时打
    // "Cannot execute ark file 'entry/ets/pages/Index.abc'"，那时拿不到原因，
    // 所以判据必须落在构建期。
    {
        let source_root = project_dir.join("src");
        if let Err(problem) = crate::build::imports::check(&source_root) {
            return Err(format!("具名 import 解析失败:\n{}", problem));
        }
    }

    let packages = crate::build::deps::npm::discover(&app_config, project_dir)?;
    let es2abc = crate::build::ets_assemble::sdk_es2abc()?;
    let kit_configs = crate::build::ets_assemble::sdk_kit_configs()?;
    let ts_root = std::env::temp_dir().join(format!("hvolt-ets-{}", std::process::id()));
    // The staging tree always starts empty. The process id is reused, so a directory left by an
    // earlier run can carry another project's modules; merging them into this build produced
    // `Variable 'X' has already been declared` for files this project never contained.
    // `HVOLT_KEEP_TS` only decides whether the tree survives this build, not whether it is
    // cleared first.
    let _ = std::fs::remove_dir_all(&ts_root);
    std::fs::create_dir_all(&ts_root).map_err(|e| format!("mkdir {}: {}", ts_root.display(), e))?;
    // One abc per package, at each package's own `ets/modules.abc`.
    let mut abc_paths: Vec<std::path::PathBuf> = Vec::new();
    for package in &planned {
        let abc_dir = package.out_dir.join("ets");
        std::fs::create_dir_all(&abc_dir)
            .map_err(|e| format!("mkdir {}: {}", abc_dir.display(), e))?;
        abc_paths.push(abc_dir.join("modules.abc"));
    }
    let abc_packages: Vec<crate::build::ets_assemble::AbcPackage<'_>> = planned
        .iter()
        .zip(&abc_paths)
        .zip(&compile_contexts)
        .map(
            |((package, out_abc), context)| crate::build::ets_assemble::AbcPackage {
                entry_module: &package.name,
                modules: &package.abc_modules,
                out_abc,
                compile_context_info: Some(context),
            },
        )
        .collect();
    crate::build::ets_assemble::assemble_packages(
        &stage_refs,
        &ts_root,
        &abc_packages,
        &crate::build::ets_assemble::AssembleOptions {
            bundle: &app_config.app.bundle,
            version: &app_config.app.version,
            version_code: app_config.app.version_code(),
            bundle_type: &app_config.app.bundle_type,
            // The product this build resolves; it names the profile entry and the package in
            // `pack.info` (`build_and_sign`, `pack_info`).
            product: app_config
                .resolved
                .as_ref()
                .map(|product| product.name.as_str())
                .unwrap_or("default"),
            // The module target hvigor names in the generated profile; hvolt has one target per
            // module and it is always `default`.
            target: "default",
            // `hvolt build` produces a debuggable package, like the reference's default product:
            // hvigor's `getBuildModeName()` is `debug` for it and `isDebug()` is true.
            build_mode: "debug",
            // app.json5 `buildVersion`: DevEco leaves it out for this project, and hvigor then
            // omits the export; an empty string in `hvolt.toml` means "not declared".
            build_version: (!app_config.app.build_version.is_empty())
                .then_some(app_config.app.build_version.as_str()),
            kit_configs_dir: &kit_configs,
            es2abc: &es2abc,
            compile_context_info: None,
            packages: &packages,
            target_api_version: app_config.app.target_api_version(),
            debug: true,
        },
    )?;

    // 5) build steps: `.rhai` scripts that read and write the project tree.
    //
    // They run after assembly and before packing so a step can adjust what gets packed,
    // and a failure stops the build before any package is written.
    if !app_config.app.build.steps.is_empty() {
        let primary = planned
            .first()
            .map(|package| package.out_dir.clone())
            .unwrap_or_else(|| out_root.clone());
        let module_name = planned
            .first()
            .map(|package| package.name.clone())
            .unwrap_or_else(|| "entry".to_string());
        let ctx = crate::build::steps::StepContext {
            project_dir,
            out_dir: &primary,
            module: &module_name,
            variant: &product_name,
        };
        let emitted = crate::build::steps::run_phase(&ctx, &app_config.app.build.steps, "post")
            .map_err(|e| format!("build step failed: {}", e))?;
        // A step that claimed artefacts narrows the pack set to them; a step that claimed
        // nothing leaves the directory as the whole input, which is the old behaviour.
        if !emitted.is_empty() {
            println!(
                "  steps emitted {} file(s), packing only those",
                emitted.len()
            );
        }
    }

    // 6) pack each package from its own output directory
    let build_dir = project_dir.join("build");
    for package in &planned {
        // The launcher and the installer read `media/layered_image.json` (base and dark) and
        // `profile/form_config.json` verbatim: their `$<kind>:<name>` references have to be the
        // compiled ids, which is what the reference's packer substitutes before archiving.
        resolve_json_references(&package.out_dir)?;
        let mut inputs = crate::build::artifacts::hap_pack::inputs_from_hvolt_out(&package.out_dir);
        // `rpcid.sc` belongs to the entry package of an OpenHarmony target; a HarmonyOS
        // multi-package build carries none (the reference's packages have none —
        // `truth/{entry,shared}/contents.txt`), while the HarmonyOS single-package baseline
        // keeps it (hello-world). A file left in the output directory by an older build must
        // not be packed as if it belonged here.
        if multi_package && !crate::support::config::is_open_harmony(&app_config.app.runtime_os) {
            inputs.rpcid_sc = None;
        }
        // Only the libraries this build produced for this package go in.
        inputs.lib_files = Some(libs_by_out.remove(&package.out_dir).unwrap_or_default());
        let unsigned = build_dir.join(format!("{}-unsigned.{}", package.name, package.extension()));
        crate::build::artifacts::hap_pack::pack(&inputs, &unsigned)?;
        println!("✓ packed {}", unsigned.display());
    }

    Ok((app_config, sdk_env))
}

/// Install a package over the hdc wire protocol, for TCP targets only.
/// Substitute `$<kind>:<name>` with `$<kind>:<id>` inside every JSON under `resources/`.
///
/// The launcher reads `media/layered_image.json` verbatim; with name references it renders a
/// broken icon. The installer reads `profile/form_config.json` the same way, where name
/// references leave every widget unnamed. The reference's packer compiles every resource
/// reference it archives, so this walks all of `resources/` rather than one media directory.
fn resolve_json_references(out_dir: &std::path::Path) -> Result<(), String> {
    let table_path = out_dir.join("res/ResourceTable.txt");
    let Ok(text) = std::fs::read_to_string(&table_path) else {
        return Ok(());
    };
    let ids = crate::build::compile::ets_resources::parse_table(&text);
    let mut files = Vec::new();
    collect_json_files(&out_dir.join("resources"), &mut files);
    for path in files {
        rewrite_json_references(&path, &ids)?;
    }
    Ok(())
}

/// Collect every `*.json` below `dir`, depth first.
fn collect_json_files(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_json_files(&path, out);
        } else if path.extension().map(|e| e == "json").unwrap_or(false) {
            out.push(path);
        }
    }
}

/// Rewrite one file's `$<kind>:<name>` references to their compiled ids, in place.
fn rewrite_json_references(
    path: &std::path::Path,
    ids: &crate::build::compile::ets_resources::ResourceTable,
) -> Result<(), String> {
    let Ok(body) = std::fs::read_to_string(path) else {
        return Ok(());
    };
    if !body.contains('$') {
        return Ok(());
    }
    // Take the whole reference name (`background_white_medium`) before looking it up:
    // replacing `$media:background_white` first would leave a dangling `_medium`.
    let mut out = String::with_capacity(body.len());
    let mut rest = body.as_str();
    while let Some(pos) = rest.find('$') {
        out.push_str(&rest[..pos]);
        let after = &rest[pos..];
        let Some((kind, tail)) = split_reference(after) else {
            out.push('$');
            rest = &after[1..];
            continue;
        };
        let name_len = tail
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '.'))
            .unwrap_or(tail.len());
        let name = &tail[..name_len];
        out.push('$');
        out.push_str(kind);
        out.push(':');
        // `$string:16777380` is already compiled — only names need a lookup.
        if !name.is_empty() && name.chars().all(|c| c.is_ascii_digit()) {
            out.push_str(name);
        } else {
            match ids.get(&(kind.to_string(), name.to_string())) {
                Some(id) => out.push_str(&id.to_string()),
                None => out.push_str(name),
            }
        }
        rest = &tail[name_len..];
    }
    out.push_str(rest);
    if out != body {
        // The rewritten file is what gets packed: a silent failure here ships a
        // `layered_image.json` still holding resource names, i.e. a broken icon.
        std::fs::write(path, out).map_err(|e| format!("write {}: {e}", path.display()))?;
    }
    Ok(())
}

/// Split `$<kind>:<rest>` into the kind and everything after its colon.
///
/// Returns `None` for a `$` that does not start a resource reference, so a stray dollar
/// sign in a string is copied through untouched.
fn split_reference(text: &str) -> Option<(&str, &str)> {
    let text = text.strip_prefix('$')?;
    let colon = text.find(':')?;
    let kind = &text[..colon];
    if kind.is_empty() || !kind.chars().all(|c| c.is_ascii_lowercase()) {
        return None;
    }
    Some((kind, &text[colon + 1..]))
}

fn native_install(target: &str, hap: &std::path::Path) -> Result<(), String> {
    let identity = crate::device::hdc_proto::HostIdentity::load_from_home()?;
    let timeout = std::time::Duration::from_secs(120);
    crate::device::hdc_proto::install(target, "Ver: 3.2.0", &identity, hap, timeout).map(|_| ())
}

/// Build + sign only (no push). Used by `hvolt build`.
/// Err is returned (caller must propagate exit code) — signing failure is a failure.
///
/// `product` is `--product`; None selects the config's `default_product`, then `default`.
pub fn build_and_sign(project_dir: &Path, product: Option<&str>) -> Result<(), String> {
    let (app_config, sdk_env) = native_build(project_dir, product)?;
    let product_name = app_config
        .resolved
        .as_ref()
        .map(|product| product.name.clone())
        .unwrap_or_else(|| "default".to_string());

    let Some(signing) = app_config.app.signing_for(&product_name) else {
        println!("  (no signing config, skipping sign)");
        return Ok(());
    };

    // DevEco encodes the signing passwords in build-profile.json5. Running SignHap with them
    // fails at the last step with a message about the keystore, which reads like a broken
    // certificate; the real answer is "these are not plaintext", so say it before signing and
    // leave the unsigned package in place.
    if signing.passwords_look_encoded() {
        println!(
            "  (signing skipped: store_password/key_password in hvolt.toml look DevEco-encoded, \
             not plaintext — replace them with the real keystore passwords)"
        );
        println!("  unsigned HAP left in build/");
        return Ok(());
    }

    crate::sign::package::sign_app(
        &sdk_env,
        project_dir,
        &product_name,
        &signing.store_file,
        &signing.store_password,
        &signing.key_alias,
        &signing.key_password,
        &signing.certpath,
        &signing.profile,
        &signing.sign_alg,
    )
    .map_err(|e| format!("Sign failed: {}", e))
}

/// Full pipeline: build + sign + push + launch. Used by `hvolt run`.
/// Report a push/install failure.
///
/// The attribution order only helps once an install actually reached the device. An unsigned
/// package or an absent device fails before that, and appending "check the certificate chain"
/// to those sends the reader down the wrong path.
pub(crate) fn report_push_failure(e: String) -> ! {
    eprintln!("✗ {}", e);
    if e.contains("INSTALL_") || e.contains("failed to install") {
        eprintln!(
            "  → order to check: signature trust (is the chain preinstalled on this device) \
             → bundle identity (does the installed version share this signature) → profile \
             validity → device constraints (space / disabled package manager)"
        );
        eprintln!(
            "    runtime permission failures are not in this list — they come from the API, \
             module declarations or ACL, not from re-signing"
        );
    }
    crate::support::cli_exit(1);
}

pub fn run(project_dir: &Path, device: Option<&str>) {
    // Without a signing config `run` cannot sign, and building first would only make the
    // package `hvolt sign --local` just produced look stale — `run` → build → "older than the
    // current build" → sign → run … Pushing what exists is the only path that terminates.
    let has_signing = AppConfig::from_file(&project_dir.join("hvolt.toml"))
        .and_then(|cfg| cfg.with_product(cfg.default_product_name()))
        .map(|cfg| !cfg.app.signing.is_empty())
        .unwrap_or(false);
    if !has_signing {
        match find_signed_packages(project_dir) {
            Ok(packages) => {
                // Every package is only usable if the tree has not moved since it was signed;
                // that check lives in `find_signed_packages`, which refuses a stale bundle.
                // Only the bundle name is needed here; building would rewrite the unsigned
                // package and make this one stale again on the next `run`.
                let bundle = AppConfig::from_file(&project_dir.join("hvolt.toml"))
                    .and_then(|cfg| cfg.with_product(cfg.default_product_name()))
                    .map(|cfg| cfg.app.bundle)
                    .unwrap_or_default();
                install_packages(&packages, device).unwrap_or_else(|e| report_push_failure(e));
                launch(device, &bundle, "EntryAbility");
                return;
            }
            Err(error) => {
                eprintln!("✗ {error}");
                if cfg!(feature = "agc-local") {
                    eprintln!("  → run `hvolt build`, `hvolt sign`, then `hvolt push`.");
                } else {
                    // Name the whole chain: the reader may be at any step of it.
                    eprintln!(
                        "  → this build has no AppGallery Connect backend: `hvolt build`, \
                         `hvolt sign --local`, then `hvolt push` (development boards and \
                         emulators only), or add an [app.signing] table to hvolt.toml."
                    );
                }
                crate::support::cli_exit(1);
            }
        }
    }

    let (app_config, sdk_env) = native_build(project_dir, None).unwrap_or_else(|e| {
        eprintln!("✗ {}", e);
        crate::support::cli_exit(1);
    });
    let product_name = app_config
        .resolved
        .as_ref()
        .map(|product| product.name.clone())
        .unwrap_or_else(|| "default".to_string());

    let Some(signing) = app_config.app.signing_for(&product_name) else {
        // `hvolt sign --local` writes a signed package but installs no config, so pointing
        // at it alone was a loop: run → sign --local → run → the same error. Pushing the
        // package it produced is what breaks that loop.
        //
        // The error must not be reduced to "no signed package": `find_signed_packages` also
        // refuses a package that predates the build it just made, and the user needs to hear
        // "re-sign" rather than "you have nothing".
        match find_signed_packages(project_dir) {
            Ok(packages) => {
                install_packages(&packages, device).unwrap_or_else(|e| report_push_failure(e));
                return;
            }
            Err(error) => eprintln!("✗ {error}"),
        }
        if cfg!(feature = "agc-local") {
            eprintln!("  → run `hvolt build`, `hvolt sign`, then `hvolt push`.");
        } else {
            eprintln!(
                "  → this build has no AppGallery Connect backend: `hvolt build`, \
                 `hvolt sign --local`, then `hvolt push` (development boards and emulators \
                 only), or add an [app.signing] table to hvolt.toml."
            );
        }
        crate::support::cli_exit(1);
    };
    if signing.passwords_look_encoded() {
        eprintln!(
            "✗ Signing password looks DevEco-encoded (hex), not plaintext — `hvolt run` needs a \
             signed package. Replace store_password/key_password in hvolt.toml."
        );
        crate::support::cli_exit(1);
    }
    if let Err(e) = crate::sign::package::sign_app(
        &sdk_env,
        project_dir,
        &product_name,
        &signing.store_file,
        &signing.store_password,
        &signing.key_alias,
        &signing.key_password,
        &signing.certpath,
        &signing.profile,
        &signing.sign_alg,
    ) {
        eprintln!("✗ Sign failed: {}", e);
        crate::support::cli_exit(1);
    }

    if let Err(e) = push_app(project_dir, device) {
        report_push_failure(e);
    }

    launch(device, &app_config.app.bundle, "EntryAbility");
}

/// Map known hdc install error codes to readable hints.
#[allow(dead_code)] // used by the remote install path only
pub(crate) fn explain_install_error(output: &str) -> String {
    // Extract the [Fail] line(s) — they contain the actual error
    let fail_lines: Vec<&str> = output.lines().filter(|l| l.contains("[Fail]")).collect();

    // Extract the error code from any line
    let error_code = output.lines().find_map(|l| {
        let l = l.trim();
        let start = l.find("error:")?;
        let err = &l[start..];
        // Grab the error code like INSTALL_FAILED_xxx or INSTALL_PARSE_FAILED_xxx
        let code = err.split_whitespace().last().unwrap_or(err);
        Some(code.to_string())
    });

    let hint = error_code.as_deref().and_then(|code| match code {
        c if c.contains("INSTALL_FAILED_SIGNATURE_ERROR") => {
            Some("  Hint: signature mismatch or certificate expired. Run `hvolt sign` to renew.")
        }
        c if c.contains("INSTALL_FAILED_INVALID_APK") => {
            Some("  Hint: invalid HAP format — try `hvolt build` to rebuild.")
        }
        c if c.contains("INSTALL_PARSE_FAILED_USESDK_ERROR") => {
            Some("  Hint: SDK version mismatch — check compile_sdk_version in hvolt.toml.")
        }
        c if c.contains("INSTALL_FAILED_NO_SIGNATURE") => {
            Some("  Hint: HAP is not signed. Run `hvolt sign` first.")
        }
        c if c.contains("INSTALL_FAILED_BUNDLE_PARSE_FAILURE") => {
            Some("  Hint: bundle parse failure — check module.json5 configuration.")
        }
        c if c.contains("INSTALL_FAILED_VERSION_DOWNGRADE") => {
            Some("  Hint: version downgrade not allowed — bump versionCode in hvolt.toml.")
        }
        c if c.contains("INSTALL_FAILED_APP_SOURCE_NOT_TRUSTED") => {
            Some("  Hint: app source not trusted — check signing config.")
        }
        _ => None,
    });

    let mut msg = String::new();
    if !fail_lines.is_empty() {
        msg.push_str(&fail_lines.join("\n"));
        if let Some(code) = &error_code {
            if !msg.contains(code) {
                msg.push_str(&format!("\n  Error: {}", code));
            }
        }
    } else if let Some(code) = &error_code {
        msg.push_str(&format!("Error: {}", code));
    } else {
        msg.push_str(output.trim());
    }
    if let Some(h) = hint {
        msg.push_str(&format!("\n{}", h));
    }
    msg
}

/// Push every signed package of the project, in bundle order: the entry HAP first, then each
/// shared package (`.hsp`) that was signed beside it.
pub fn push_app(project_dir: &Path, device: Option<&str>) -> Result<(), String> {
    let packages = find_signed_packages(project_dir)?;
    install_packages(&packages, device)
}

/// Install the package the caller points at: a bare `.hap`/`.hsp` that lives outside any
/// project `output/`, so nothing is resolved relative to the working directory.
///
/// An unsigned package fails here rather than at hdc: the device would only report a signature
/// error, which says nothing about the fix (`hvolt sign <package>`).
pub fn push_package(hap: &Path, device: Option<&str>) -> Result<(), String> {
    if !hap.exists() {
        return Err(format!("{} not found", hap.display()));
    }
    if !crate::project::hap::is_signed(hap)? {
        return Err(format!(
            "{} is not signed — run `hvolt sign {}` first",
            hap.display(),
            hap.display()
        ));
    }
    install_package(hap, device)
}

/// Install packages one after another. An install refuses a second package while one is in
/// flight, so the calls are strictly sequential; a failure names the package it stopped on.
fn install_packages(packages: &[std::path::PathBuf], device: Option<&str>) -> Result<(), String> {
    if packages.is_empty() {
        return Err("no signed package to install".to_string());
    }
    for (index, package) in packages.iter().enumerate() {
        println!("  [{}/{}] {}", index + 1, packages.len(), package.display());
        install_package(package, device).map_err(|e| {
            format!(
                "install {} ({}/{}) failed: {e}",
                package.file_name().unwrap_or_default().to_string_lossy(),
                index + 1,
                packages.len()
            )
        })?;
    }
    Ok(())
}

fn install_package(package: &Path, device: Option<&str>) -> Result<(), String> {
    let target = crate::device::target::resolve_target(device)?;
    println!("  push target: {}", target);
    native_install(&target, package)?;
    println!(
        "✓ Installed {}",
        package.file_name().unwrap_or_default().to_string_lossy()
    );
    Ok(())
}

fn launch(device: Option<&str>, bundle: &str, entry: &str) {
    let target = match crate::device::target::resolve_target(device) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("✗ {e}");
            crate::support::cli_exit(1);
        }
    };
    let command = format!("aa start -a {entry} -b {bundle}");
    let identity = match crate::device::hdc_proto::HostIdentity::load_from_home() {
        Ok(i) => i,
        Err(e) => {
            eprintln!("✗ {e}");
            crate::support::cli_exit(1);
        }
    };
    match crate::device::hdc_proto::shell(
        &target,
        "Ver: 3.2.0",
        &identity,
        &command,
        std::time::Duration::from_secs(60),
    ) {
        Ok(text) if !text.to_lowercase().contains("error") => {
            println!("✓ App launched: {}.{}", bundle, entry);
        }
        Ok(text) => {
            eprintln!("✗ Launch failed: {}", text.trim());
            crate::support::cli_exit(1);
        }
        Err(e) => {
            eprintln!("✗ Launch failed: {e}");
            crate::support::cli_exit(1);
        }
    }
}

/// Locate the signed hap. The native chain packs into `build/`
/// (`<module>-signed.hap`) and `hvolt build` copies the signed result to
/// `output/`; both are checked, newest by mtime wins, `output/` first so the
/// published artifact is preferred when both exist.
/// Whether any source file is newer than `package`.
///
/// `run` without a signing config cannot rebuild: rebuilding would rewrite the unsigned
/// package and make the signed one look stale on the next call. It therefore pushes the
/// existing package — and must know when that package no longer matches the tree.
pub(crate) fn sources_newer_than(project_dir: &std::path::Path, package: &std::path::Path) -> bool {
    let Ok(package_at) = package.metadata().and_then(|meta| meta.modified()) else {
        return false;
    };
    let mut newest = None;
    let mut stack = vec![project_dir.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("");
            // Build output is not a source; anything else is.
            if name == "build" || name == "output" || name == ".git" || name == "oh_modules" {
                continue;
            }
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if let Ok(modified) = path.metadata().and_then(|meta| meta.modified()) {
                newest = Some(newest.map_or(modified, |newest: std::time::SystemTime| {
                    newest.max(modified)
                }));
            }
        }
    }
    newest.is_some_and(|newest| newest > package_at)
}

/// Directories a build may have written a package to: `output/`, each `output/<product>/`,
/// then `build/`. A non-default product's package lives one level deeper, so the finders have
/// to look there too; the newest file still wins, as it does inside one directory.
fn package_dirs(project_dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let output = project_dir.join("output");
    let mut dirs = vec![output.clone()];
    if let Ok(entries) = std::fs::read_dir(&output) {
        let mut products: Vec<std::path::PathBuf> = entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.is_dir())
            .collect();
        products.sort();
        dirs.extend(products);
    }
    dirs.push(project_dir.join("build"));
    dirs
}

/// Extension of a package file, lower-case (`hap`, `hsp`, …).
fn package_extension(path: &std::path::Path) -> String {
    path.extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
}

/// The newest `*-unsigned.<extension>` a build produced.
fn newest_unsigned(project_dir: &std::path::Path, extension: &str) -> Option<std::path::PathBuf> {
    let suffix = format!("-unsigned.{extension}");
    let mut found = Vec::new();
    for dir in package_dirs(project_dir) {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("");
            if name.ends_with(&suffix) {
                found.push(path);
            }
        }
    }
    found.sort_by_key(|path| path.metadata().and_then(|meta| meta.modified()).ok());
    found.pop()
}

/// The newest `*-unsigned.hap` a build produced, for a caller that has to explain why the
/// signed one is missing.
pub(crate) fn find_unsigned_hap(project_dir: &std::path::Path) -> Option<std::path::PathBuf> {
    newest_unsigned(project_dir, "hap")
}

/// The signed packages a build produced, in install order: entry HAPs first, then shared
/// packages (`.hsp`). One file per name (the newest copy) so a product directory and `build/`
/// do not yield the same package twice.
fn signed_candidates(project_dir: &std::path::Path, suffix: &str) -> Vec<std::path::PathBuf> {
    let mut cands: Vec<std::path::PathBuf> = Vec::new();
    for dir in package_dirs(project_dir) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        cands.extend(
            entries
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| {
                    p.is_file()
                        && p.file_name()
                            .map(|n| n.to_string_lossy().ends_with(suffix))
                            .unwrap_or(false)
                }),
        );
    }
    // Newest first, then keep the first copy of each file name.
    cands.sort_by(|a, b| {
        b.metadata()
            .and_then(|m| m.modified())
            .ok()
            .cmp(&a.metadata().and_then(|m| m.modified()).ok())
    });
    let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    cands.retain(|path| {
        seen.insert(
            path.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
        )
    });
    cands
}

/// Refuse a signed package that no longer matches its build: a package older than the newest
/// unsigned one was built from an earlier tree, and pushing it installs code the user already
/// changed, with no output saying so. Re-signing alone does not help — it would stamp a new
/// mtime onto the same pre-edit package — so the message starts at `build`.
fn check_signed_package(
    project_dir: &std::path::Path,
    package: &std::path::Path,
    unsigned: Option<&std::path::Path>,
) -> Result<(), String> {
    let signed_at = package.metadata().and_then(|meta| meta.modified()).ok();
    let unsigned_at =
        unsigned.and_then(|path| path.metadata().and_then(|meta| meta.modified()).ok());
    if let (Some(signed_at), Some(unsigned_at), Some(unsigned)) = (signed_at, unsigned_at, unsigned)
    {
        if signed_at < unsigned_at {
            return Err(format!(
                "{} is older than {}: run `hvolt build` and then `hvolt sign --local` \
                 (or add an [app.signing] table) so the package matches the sources.",
                package.display(),
                unsigned.display()
            ));
        }
    }
    // Compare the sources against the *build*, not against the signature: re-signing refreshes
    // the signed mtime without rebuilding, so a signed-vs-sources check passes exactly when the
    // package is stale.
    if let Some(unsigned) = unsigned {
        if sources_newer_than(project_dir, unsigned) {
            return Err(format!(
                "{} predates the current sources: run `hvolt build` and then \
                 `hvolt sign --local`.",
                unsigned.display()
            ));
        }
    }
    Ok(())
}

/// Every signed package a build produced, in install order: entry HAPs first, then HSPs.
pub fn find_signed_packages(
    project_dir: &std::path::Path,
) -> Result<Vec<std::path::PathBuf>, String> {
    let mut packages = signed_candidates(project_dir, "-signed.hap");
    packages.extend(signed_candidates(project_dir, "-signed.hsp"));
    if packages.is_empty() {
        // A build with no `[app.signing]` produces `*-unsigned.*` only; telling that user to
        // "run hvolt build" sends them round the loop they just finished.
        return Err(match find_unsigned_hap(project_dir) {
            // `build` first: signing an unsigned package that predates the sources produces a
            // package whose mtime is new and whose code is old.
            Some(unsigned) => format!(
                "{} is unsigned: run `hvolt build`, then `hvolt sign --local` (development \
                 boards), then `hvolt push` — or add an [app.signing] table and build again",
                unsigned.display()
            ),
            None => "no *-signed.hap or *-signed.hsp in output/, output/<product>/ or build/ — \
                     run `hvolt build` first"
                .to_string(),
        });
    }
    for package in &packages {
        let unsigned = newest_unsigned(project_dir, &package_extension(package));
        check_signed_package(project_dir, package, unsigned.as_deref())?;
    }
    Ok(packages)
}

/// The newest signed HAP, for callers that inspect one package (`hvolt hap info`, localSign
/// verification) and never install a bundle.
pub fn find_signed_hap(project_dir: &std::path::Path) -> Result<std::path::PathBuf, String> {
    let mut packages = find_signed_packages(project_dir)?;
    if let Some(index) = packages
        .iter()
        .position(|package| package_extension(package) == "hap")
    {
        return Ok(packages.remove(index));
    }
    Err(
        "no *-signed.hap in output/, output/<product>/ or build/ — run `hvolt build` first"
            .to_string(),
    )
}

#[cfg(test)]
mod install_error_tests {
    use super::explain_install_error;

    /// Signature problems must point at the command that fixes them.
    #[test]
    fn signature_errors_point_at_signing() {
        assert!(
            explain_install_error("error: INSTALL_FAILED_SIGNATURE_ERROR").contains("hvolt sign")
        );
        assert!(explain_install_error("error: INSTALL_FAILED_NO_SIGNATURE").contains("not signed"));
    }

    /// A downgrade is fixed by bumping versionCode, so the hint must name it.
    #[test]
    fn version_downgrade_names_version_code() {
        let msg = explain_install_error("error: INSTALL_FAILED_VERSION_DOWNGRADE");
        assert!(msg.contains("versionCode"));
    }

    /// [Fail] lines carry the real reason and are kept; unrelated progress noise is dropped.
    #[test]
    fn fail_lines_are_preserved_and_noise_dropped() {
        let out = "[Fail]Error: failed to install bundle\nprogress: 40%\nerror: INSTALL_FAILED_INVALID_APK";
        let msg = explain_install_error(out);
        assert!(msg.contains("[Fail]Error: failed to install bundle"));
        assert!(msg.contains("hvolt build"));
        assert!(!msg.contains("progress: 40%"));
    }

    /// An unknown code is surfaced, but no hint is invented for it.
    #[test]
    fn unknown_codes_surface_without_a_hint() {
        let msg = explain_install_error("error: INSTALL_SOMETHING_NEW");
        assert!(msg.contains("INSTALL_SOMETHING_NEW"));
        assert!(!msg.contains("Hint:"));
    }

    /// No code, no [Fail] line: pass the trimmed output through and never panic.
    #[test]
    fn unrecognised_output_is_passed_through() {
        assert_eq!(
            explain_install_error("  plain failure text  \n"),
            "plain failure text"
        );
        assert_eq!(explain_install_error(""), "");
    }
}

#[cfg(test)]
mod signed_package_tests {
    use super::find_signed_packages;

    /// The entry HAP is installed before the shared package: a bundle that has only the HSP is
    /// not installable, and the installer refuses a second package while one is in flight.
    #[test]
    fn hap_is_ordered_before_hsp() {
        let root = std::env::temp_dir().join(format!("hvolt-pkg-order-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let build = root.join("build");
        std::fs::create_dir_all(&build).unwrap();
        std::fs::write(build.join("box64hsp-signed.hsp"), b"").unwrap();
        std::fs::write(build.join("src-signed.hap"), b"").unwrap();
        let packages = find_signed_packages(&root).unwrap();
        let names: Vec<String> = packages
            .iter()
            .map(|path| {
                path.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string()
            })
            .collect();
        assert_eq!(names, vec!["src-signed.hap", "box64hsp-signed.hsp"]);
        let _ = std::fs::remove_dir_all(&root);
    }
}
