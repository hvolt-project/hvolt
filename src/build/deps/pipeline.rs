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
    modules: &[(String, crate::support::config::ModuleConfig)],
    out_dir: &Path,
) -> Result<Vec<std::path::PathBuf>, String> {
    let libs = out_dir.join("arm64-v8a");
    std::fs::create_dir_all(&libs).map_err(|e| format!("mkdir {}: {e}", libs.display()))?;
    let mut copied = Vec::new();
    for (name, _) in modules {
        let source = project_dir.join(name).join("libs").join("arm64-v8a");
        let Ok(entries) = std::fs::read_dir(&source) else {
            continue;
        };
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
    modules: &[(String, crate::support::config::ModuleConfig)],
    out_dir: &Path,
) -> Result<Vec<std::path::PathBuf>, String> {
    let libs = out_dir.join("arm64-v8a");
    std::fs::create_dir_all(&libs).map_err(|e| format!("mkdir {}: {e}", libs.display()))?;
    let mut produced = Vec::new();
    for (name, _) in modules {
        let module_dir = std::path::absolute(project_dir.join(name).join("cpp"))
            .map_err(|e| format!("absolute {name}: {e}"))?;
        if !module_dir.join("CMakeLists.txt").exists() {
            continue;
        }
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
        crate::build::artifacts::native_compile::strip(sdk_root, &built)?;
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

fn native_build(project_dir: &Path) -> Result<(AppConfig, SdkEnv), String> {
    let root_toml = project_dir.join("hvolt.toml");
    let app_config = AppConfig::from_file(&root_toml)?;

    let sdk_env = crate::sdk::manage::detect(app_config.app.sdk_path.as_deref().unwrap_or(""))
        .map_err(|e| format!("SDK not available: {}", e))?;
    println!("  SDK detected: {}", sdk_env.ohos_sdk.display());

    // 1) native libs → build/hvolt-out/libs
    let (modules, native) = crate::translator::discover_modules(project_dir, &app_config)
        .map_err(|e| format!("discover modules: {e}"))?;
    let out_dir = project_dir.join("build/hvolt-out");
    let sdk_root =
        crate::sdk::manage::sdk_root().ok_or_else(|| "no SDK root resolved".to_string())?;
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
    let produced = crate::build::artifacts::native_compile::build_all(
        &sdk_root,
        &native_names,
        &out_dir.join("libs"),
    )?;
    let built_native =
        build_module_native(&sdk_root, project_dir, &modules, &out_dir.join("libs"))?;
    if !built_native.is_empty() {
        println!("✓ native lib(s) from <module>/cpp: {}", built_native.len());
    }
    let libs_dir = out_dir.join("libs");
    // The C++ runtime every native module links against. Without it a C++ module's exports
    // read `undefined` at import time (see `cxx_runtime`).
    if let Some(runtime) = crate::build::artifacts::native_compile::cxx_runtime(&sdk_root) {
        let abi_dir = libs_dir.join("arm64-v8a");
        std::fs::create_dir_all(&abi_dir)
            .map_err(|e| format!("mkdir {}: {e}", abi_dir.display()))?;
        let dest = abi_dir.join("libc++_shared.so");
        std::fs::copy(&runtime, &dest).map_err(|e| format!("copy {}: {e}", runtime.display()))?;
    }
    let prebuilt = collect_prebuilt_libs(project_dir, &modules, &libs_dir)?;
    if !prebuilt.is_empty() {
        println!("✓ prebuilt lib(s) copied: {}", prebuilt.len());
    }
    if !produced.is_empty() {
        println!("✓ native lib(s) from libs/: {}", produced.len());
    }

    // 2) resources + module metadata under the same hvolt-out
    let module_arg = modules.first().map(|(n, _)| n.clone()).unwrap_or_default();
    crate::build::artifacts::assemble_hap::run(project_dir, &module_arg, Some(&out_dir))?;

    // 3) modules.abc from the module's .ets tree
    // Record names carry the module's logical name, not its directory: the device
    // resolves `<module>/src/main/ets/<srcEntry path>` and a directory-derived
    // name (`src`) never matches.
    let module_cfg = crate::support::config::ModuleConfig::from_file(
        &project_dir.join(&module_arg).join("hvolt.toml"),
    )
    .ok();
    let module_name = module_cfg
        .as_ref()
        .map(|m| m.module.name.clone())
        .unwrap_or_else(|| module_arg.clone());
    let entry_file = module_cfg
        .as_ref()
        .map(|m| m.module.entry.clone())
        .unwrap_or_else(|| "EntryAbility.ets".to_string());
    let ets_dir = project_dir.join(&module_arg);
    /// One staged module: name, dir, entry file, compile roots, is-HAR, declared `main`.
    type Staged = (
        String,
        std::path::PathBuf,
        String,
        Vec<String>,
        bool,
        Option<String>,
    );
    // Every ETS module in the project shares one merged abc: an import from the entry
    // module names a dependency module, so leaving it out makes the record unresolvable.
    // The entry module comes first, the rest follow in dependency order.
    //
    // The entry module compiles the sources the reference hands to rollup: every page route,
    // every ability and the module's own entry, plus whatever those import. The other modules
    // have their whole tree compiled, which is what the reference puts into `compileEntry` for
    // an HSP/HAR (`generate-loader-json.js`: `allowToCollectEtsFolder`).
    let mut staged: Vec<Staged> = vec![(
        module_name.clone(),
        ets_dir.clone(),
        entry_file.clone(),
        compile_roots(module_cfg.as_ref(), &ets_dir)?,
        module_cfg
            .as_ref()
            .is_some_and(|cfg| cfg.module.module_type == "har"),
        module_cfg.as_ref().and_then(|cfg| cfg.module.main.clone()),
    )];
    for (name, cfg) in &modules {
        if name == &module_arg {
            continue;
        }
        staged.push((
            cfg.module.name.clone(),
            project_dir.join(name),
            cfg.module.entry.clone(),
            Vec::new(),
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
    // `--compile-context-info` wants the wrapper shape the reference emits, not the raw
    // table: `{ compileEntries, pkgContextInfo }`.
    let compile_context = std::fs::canonicalize(&out_dir)
        .unwrap_or_else(|_| out_dir.clone())
        .join("compileContextInfo.json");
    {
        let pkg_context = std::fs::read_to_string(out_dir.join("pkgContextInfo.json"))
            .unwrap_or_else(|_| "{}".to_string());
        let entry_no_ext = entry_file.trim_start_matches("./").trim_end_matches(".ets");
        let entry_record =
            crate::build::compile::abc_compile::module_path(&module_name, entry_no_ext);
        let wrapped = format!(
            "{{\"compileEntries\":[\"{entry_record}\"],\"hspPkgNames\":[],\"pkgContextInfo\":{pkg_context}}}"
        );
        // The path is handed to es2abc below; a failed write there would compile against a
        // missing context file, so the failure has to surface here.
        std::fs::write(&compile_context, wrapped)
            .map_err(|e| format!("write {}: {e}", compile_context.display()))?;
    }
    // Third-party packages the app declares; their records are part of the same abc.
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
    // One merged abc per module, at `ets/modules.abc`.
    let abc_dir = out_dir.join("ets");
    std::fs::create_dir_all(&abc_dir).map_err(|e| format!("mkdir {}: {}", abc_dir.display(), e))?;
    let abc_path = abc_dir.join("modules.abc");
    crate::build::ets_assemble::assemble(
        &stage_refs,
        &ts_root,
        &abc_path,
        &crate::build::ets_assemble::AssembleOptions {
            bundle: &app_config.app.bundle,
            version: &app_config.app.version,
            version_code: app_config.app.version_code(),
            bundle_type: &app_config.app.bundle_type,
            // hvolt builds the default product: the signing config and the pack step address
            // it by that name (`build_and_sign`, `pack_info`), and the import translation
            // ignores `buildModeSet` (`project/import.rs`: "hvolt always builds the default
            // product").
            product: "default",
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
            compile_context_info: Some(&compile_context),
            packages: &packages,
            target_api_version: app_config.app.target_api_version(),
            debug: true,
        },
    )?;

    // The installer and the launcher read `media/layered_image.json` verbatim: its
    // `$media:<name>` references have to be the compiled ids, which is what the
    // reference's packer substitutes before archiving.
    resolve_media_references(&out_dir)?;

    // 4) pack the unsigned HAP from hvolt-out
    let build_dir = project_dir.join("build");
    let unsigned = build_dir.join(format!("{}-unsigned.hap", module_arg));
    let inputs = crate::build::artifacts::hap_pack::inputs_from_hvolt_out(&out_dir);
    crate::build::artifacts::hap_pack::pack(&inputs, &unsigned)?;
    println!("✓ packed {}", unsigned.display());

    Ok((app_config, sdk_env))
}

/// Install a package over the hdc wire protocol, for TCP targets only.
/// Substitute `$media:<name>` with `$media:<id>` inside every JSON under `resources/`.
///
/// The launcher reads `media/layered_image.json` verbatim; with name references it
/// renders a broken icon, while the reference packer ships compiled ids there.
fn resolve_media_references(out_dir: &std::path::Path) -> Result<(), String> {
    let table_path = out_dir.join("res/ResourceTable.txt");
    let Ok(text) = std::fs::read_to_string(&table_path) else {
        return Ok(());
    };
    let ids = crate::build::compile::ets_resources::parse_table(&text);
    let media_dir = out_dir.join("resources/base/media");
    let Ok(entries) = std::fs::read_dir(&media_dir) else {
        return Ok(());
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().map(|e| e != "json").unwrap_or(true) {
            continue;
        }
        let Ok(body) = std::fs::read_to_string(&path) else {
            continue;
        };
        if !body.contains("$media:") {
            continue;
        }
        // Take the whole reference name (`background_white_medium`) before looking it up:
        // replacing `$media:background_white` first would leave a dangling `_medium`.
        let mut out = String::with_capacity(body.len());
        let mut rest = body.as_str();
        while let Some(pos) = rest.find("$media:") {
            out.push_str(&rest[..pos + "$media:".len()]);
            let after = &rest[pos + "$media:".len()..];
            let name_len = after
                .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '.'))
                .unwrap_or(after.len());
            let name = &after[..name_len];
            match ids.get(&("media".to_string(), name.to_string())) {
                Some(id) => out.push_str(&id.to_string()),
                None => out.push_str(name),
            }
            rest = &after[name_len..];
        }
        out.push_str(rest);
        if out != body {
            // The rewritten file is what gets packed: a silent failure here ships a
            // `layered_image.json` still holding resource names, i.e. a broken icon.
            std::fs::write(&path, out).map_err(|e| format!("write {}: {e}", path.display()))?;
        }
    }
    Ok(())
}

fn native_install(target: &str, hap: &std::path::Path) -> Result<(), String> {
    let identity = crate::device::hdc_proto::HostIdentity::load_from_home()?;
    let timeout = std::time::Duration::from_secs(120);
    crate::device::hdc_proto::install(target, "Ver: 3.2.0", &identity, hap, timeout).map(|_| ())
}

/// Build + sign only (no push). Used by `hvolt build`.
/// Err is returned (caller must propagate exit code) — signing failure is a failure.
pub fn build_and_sign(project_dir: &Path) -> Result<(), String> {
    let (app_config, sdk_env) = native_build(project_dir)?;

    let Some(signing) = app_config.app.signing_for("default") else {
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
    std::process::exit(1);
}

pub fn run(project_dir: &Path, device: Option<&str>) {
    // Without a signing config `run` cannot sign, and building first would only make the
    // package `hvolt sign --local` just produced look stale — `run` → build → "older than the
    // current build" → sign → run … Pushing what exists is the only path that terminates.
    let has_signing = AppConfig::from_file(&project_dir.join("hvolt.toml"))
        .map(|cfg| cfg.app.signing_for("default").is_some())
        .unwrap_or(false);
    if !has_signing {
        match find_signed_hap(project_dir) {
            Ok(hap) => {
                // The package is only usable if the tree has not moved since it was signed;
                // otherwise this would install the code the user just replaced, silently.
                if sources_newer_than(project_dir, &hap) {
                    eprintln!(
                        "✗ {} predates the current sources. Re-sign with `hvolt sign --local`, \
                         then `hvolt push`.",
                        hap.display()
                    );
                    std::process::exit(1);
                }
                // Only the bundle name is needed here; building would rewrite the unsigned
                // package and make this one stale again on the next `run`.
                let bundle = AppConfig::from_file(&project_dir.join("hvolt.toml"))
                    .map(|cfg| cfg.app.bundle)
                    .unwrap_or_default();
                push_package(&hap, device).unwrap_or_else(|e| report_push_failure(e));
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
                std::process::exit(1);
            }
        }
    }

    let (app_config, sdk_env) = native_build(project_dir).unwrap_or_else(|e| {
        eprintln!("✗ {}", e);
        std::process::exit(1);
    });

    let Some(signing) = app_config.app.signing_for("default") else {
        // `hvolt sign --local` writes a signed package but installs no config, so pointing
        // at it alone was a loop: run → sign --local → run → the same error. Pushing the
        // package it produced is what breaks that loop.
        //
        // The error must not be reduced to "no signed package": `find_signed_hap` also
        // refuses a package that predates the build it just made, and the user needs to hear
        // "re-sign" rather than "you have nothing".
        match find_signed_hap(project_dir) {
            Ok(hap) => {
                push_package(&hap, device).unwrap_or_else(|e| report_push_failure(e));
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
        std::process::exit(1);
    };
    if signing.passwords_look_encoded() {
        eprintln!(
            "✗ Signing password looks DevEco-encoded (hex), not plaintext — `hvolt run` needs a \
             signed package. Replace store_password/key_password in hvolt.toml."
        );
        std::process::exit(1);
    }
    if let Err(e) = crate::sign::package::sign_app(
        &sdk_env,
        project_dir,
        &signing.store_file,
        &signing.store_password,
        &signing.key_alias,
        &signing.key_password,
        &signing.certpath,
        &signing.profile,
        &signing.sign_alg,
    ) {
        eprintln!("✗ Sign failed: {}", e);
        std::process::exit(1);
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

/// Push signed HAP to device via hdc (local) or scp+remote hdc (cross-machine).
pub fn push_app(project_dir: &Path, device: Option<&str>) -> Result<(), String> {
    let hap = find_signed_hap(project_dir)?;
    install_package(&hap, device)
}

/// Install the package the caller points at: a bare `.hap` that lives outside any project
/// `output/`, so nothing is resolved relative to the working directory.
///
/// An unsigned package fails here rather than at hdc: the device would only report a signature
/// error, which says nothing about the fix (`hvolt sign <HAP>`).
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

fn install_package(hap: &Path, device: Option<&str>) -> Result<(), String> {
    let target = crate::device::target::resolve_target(device)?;
    println!("  push target: {}", target);
    native_install(&target, hap)?;
    println!("✓ Installed to device");
    Ok(())
}

fn launch(device: Option<&str>, bundle: &str, entry: &str) {
    let target = match crate::device::target::resolve_target(device) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("✗ {e}");
            std::process::exit(1);
        }
    };
    let command = format!("aa start -a {entry} -b {bundle}");
    let identity = match crate::device::hdc_proto::HostIdentity::load_from_home() {
        Ok(i) => i,
        Err(e) => {
            eprintln!("✗ {e}");
            std::process::exit(1);
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
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("✗ Launch failed: {e}");
            std::process::exit(1);
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

/// The newest `*-unsigned.hap` a build produced, for a caller that has to explain why the
/// signed one is missing.
pub(crate) fn find_unsigned_hap(project_dir: &std::path::Path) -> Option<std::path::PathBuf> {
    let mut found = Vec::new();
    for dir in [project_dir.join("output"), project_dir.join("build")] {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("");
            if name.ends_with("-unsigned.hap") {
                found.push(path);
            }
        }
    }
    found.sort_by_key(|path| path.metadata().and_then(|meta| meta.modified()).ok());
    found.pop()
}

pub fn find_signed_hap(project_dir: &std::path::Path) -> Result<std::path::PathBuf, String> {
    let mut cands: Vec<std::path::PathBuf> = Vec::new();
    for dir in ["output", "build"] {
        let Ok(entries) = std::fs::read_dir(project_dir.join(dir)) else {
            continue;
        };
        cands.extend(
            entries
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| {
                    p.is_file()
                        && p.file_name()
                            .map(|n| n.to_string_lossy().ends_with("-signed.hap"))
                            .unwrap_or(false)
                }),
        );
    }
    cands.sort_by(|a, b| {
        b.metadata()
            .and_then(|m| m.modified())
            .ok()
            .cmp(&a.metadata().and_then(|m| m.modified()).ok())
    });
    if let Some(found) = cands.into_iter().next() {
        // A signed package older than the newest unsigned one was built from an earlier
        // tree; pushing it installs code the user already changed, with no output saying so.
        let newest_unsigned = find_unsigned_hap(project_dir);
        let signed_at = found.metadata().and_then(|meta| meta.modified()).ok();
        let unsigned_at = newest_unsigned
            .as_ref()
            .and_then(|path| path.metadata().and_then(|meta| meta.modified()).ok());
        if let (Some(signed_at), Some(unsigned_at), Some(unsigned)) =
            (signed_at, unsigned_at, newest_unsigned.as_ref())
        {
            if signed_at < unsigned_at {
                // Re-signing alone does not help: it would stamp a new mtime onto the same
                // pre-edit package. The message has to start at `build`.
                return Err(format!(
                    "{} is older than {}: run `hvolt build` and then `hvolt sign --local` \
                     (or add an [app.signing] table) so the package matches the sources.",
                    found.display(),
                    unsigned.display()
                ));
            }
        }
        // Compare the sources against the *build*, not against the signature: re-signing
        // refreshes the signed mtime without rebuilding, so a signed-vs-sources check passes
        // exactly when the package is stale.
        if let Some(unsigned) = newest_unsigned.as_ref() {
            if sources_newer_than(project_dir, unsigned) {
                return Err(format!(
                    "{} predates the current sources: run `hvolt build` and then \
                     `hvolt sign --local`.",
                    unsigned.display()
                ));
            }
        }
        return Ok(found);
    }
    // A build with no `[app.signing]` produces `*-unsigned.hap` only; telling that user to
    // "run hvolt build" sends them round the loop they just finished.
    Err(match find_unsigned_hap(project_dir) {
        // `build` first: signing an unsigned package that predates the sources produces a
        // package whose mtime is new and whose code is old.
        Some(unsigned) => format!(
            "{} is unsigned: run `hvolt build`, then `hvolt sign --local` (development \
             boards), then `hvolt push` — or add an [app.signing] table and build again",
            unsigned.display()
        ),
        None => "no *-signed.hap in output/ or build/ — run `hvolt build` first".to_string(),
    })
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
