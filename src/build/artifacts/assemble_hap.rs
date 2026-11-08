// assemble_hap: build the resource/configuration layer of a HAP without
// — restool-compiled resources, the id-injected module.json, rpcid.sc,
// pack.info and pkgContextInfo.json, all under `<project>/build/hvolt-out`.
//
// This is the second stage of hvolt's native (standalone, no-JDK) build chain:
// build-native → assemble-hap → assemble-ets → pack-hap → sign.

use std::path::{Path, PathBuf};

/// Compile resources and render the package metadata into `out_dir`.
///
/// Returns the resolved output directory (the caller's `out` or the default
/// `<project>/build/hvolt-out`). The product is the project's `default_product`; a caller that
/// already resolved one (the build pipeline) uses `run_with` instead.
pub fn run(project: &Path, module_arg: &str, out: Option<&Path>) -> Result<PathBuf, String> {
    let declared = crate::support::config::AppConfig::from_file(&project.join("hvolt.toml"))
        .map_err(|e| format!("read {}: {e}", project.join("hvolt.toml").display()))?;
    let app = declared.with_product(declared.default_product_name())?;
    run_with(project, module_arg, out, &app)
}

/// Compile resources and render the package metadata for an already-resolved product.
pub fn run_with(
    project: &Path,
    module_arg: &str,
    out: Option<&Path>,
    app: &crate::support::config::AppConfig,
) -> Result<PathBuf, String> {
    let (modules, native) = crate::translator::discover_modules(project, app)
        .map_err(|e| format!("discover modules: {e}"))?;
    // Third-party packages compiled into the abc also need a package context entry.
    let packages = crate::build::deps::npm::discover(app, project)?;
    // The source directory is only a lookup key here: everything the package records comes
    // from the module's declared name.
    let (module_dir, module_cfg) = if module_arg.is_empty() {
        modules
            .first()
            .map(|(n, m)| (n.clone(), m))
            .ok_or_else(|| "no modules found in hvolt.toml".to_string())?
    } else {
        modules
            .iter()
            .find(|(n, _)| n == module_arg)
            .map(|(n, m)| (n.clone(), m))
            .ok_or_else(|| format!("module {module_arg} not found"))?
    };

    // The package carries the module's declared name, not its source directory.
    let logical_name = module_cfg.module.name.clone();
    let default_out = project.join("build/hvolt-out");
    let out_dir = out.unwrap_or(&default_out);
    std::fs::create_dir_all(out_dir).map_err(|e| format!("mkdir {}: {e}", out_dir.display()))?;

    // 1) resources: restool compiles and injects the real resource ids
    //    into module.json itself, so the table is read back for the rest.
    // Values the package carries come from the SDK and the build mode, never from literals.
    let sdk_env = crate::sdk::manage::detect("")?;
    let sdk_root = sdk_env.ohos_sdk.clone();
    let node_binary = crate::sdk::manage::node_bin(&sdk_env.node_home);
    let compatible_api =
        crate::build::deps::sdk_facts::compatible_api(&app.app.compatible_sdk_version)
            .unwrap_or_default();
    let facts =
        crate::build::deps::sdk_facts::SdkFacts::detect(&sdk_root, &node_binary, compatible_api)?;
    let build_mode = "debug";

    let res_dir = out_dir.join("res");
    let table_path = crate::build::artifacts::res_compile::run(
        project,
        &project.join(&module_dir),
        module_cfg,
        app,
        &res_dir,
        build_mode,
        &facts,
    )?;
    let table_text = std::fs::read_to_string(&table_path)
        .map_err(|e| format!("read {}: {e}", table_path.display()))?;
    let ids = crate::build::compile::ets_resources::parse_table(&table_text);

    // 2) module.json: restool already wrote the id-injected copy
    //    next to its outputs; move it to the hvolt-out root.
    let restool_module = res_dir.join("module.json");
    let final_module = out_dir.join("module.json");
    std::fs::copy(&restool_module, &final_module).map_err(|e| format!("copy module.json: {e}"))?;

    // 3) rpcid.sc: encoded in-tree from the SDK device-define intersection
    let syscap_path = crate::build::artifacts::syscap::generate(
        &sdk_root,
        out_dir,
        app.app.target_api_version(),
        &module_cfg.module.device_types,
    )?;
    println!("  rpcid.sc            {}", syscap_path.display());

    // 4) lift resources.index and resources/ to the hvolt-out root
    //    (restool writes them under res/; the package layout wants
    //    them at the top, like the packaged module layout).
    let lifted_index = out_dir.join("resources.index");
    std::fs::copy(res_dir.join("resources.index"), &lifted_index)
        .map_err(|e| format!("copy resources.index: {e}"))?;
    let lifted_res = out_dir.join("resources");
    if lifted_res.exists() {
        std::fs::remove_dir_all(&lifted_res)
            .map_err(|e| format!("rm {}: {e}", lifted_res.display()))?;
    }
    copy_dir_flat(&res_dir.join("resources"), &lifted_res)?;

    // 5) pack.info from the same manifest + table
    let pack_info = crate::build::artifacts::pack_info::render(
        &crate::build::artifacts::pack_info::PackInput {
            // The package carries the module's declared name, not its source directory.
            module_name: &logical_name,
            module: module_cfg,
            app,
            product: app
                .resolved
                .as_ref()
                .map(|p| p.name.as_str())
                .unwrap_or("default"),
            build_mode,
            compile_sdk_version: &facts.version,
            api_release_type: &facts.release_type,
            virtual_machine: &facts.virtual_machine,
            has_insight_intent: crate::build::artifacts::insight_intent::collect(
                project,
                &module_cfg.module.name,
            )
            .map(|entries| !entries.is_empty())
            .unwrap_or(false),
            ids: &ids,
        },
    );
    let pack_path = out_dir.join("pack.info");
    std::fs::write(
        &pack_path,
        serde_json::to_string_pretty(&pack_info).map_err(|e| e.to_string())?,
    )
    .map_err(|e| format!("write pack.info: {e}"))?;

    // 6) pkgContextInfo.json from declared native dependencies. Keyed by the module's
    // logical name: the runtime looks the module up by that name and reports an empty
    // PkgContextInfo when the key is the source directory instead.
    let ets_deps: Vec<(String, String)> = modules
        .iter()
        .filter(|(dir, _)| dir != module_arg)
        // Package-entry path as the device composes it: `Index.ets`-style, not the toml entry.
        .map(|(_dir, cfg)| (cfg.module.name.clone(), "Index.ets".to_string()))
        .collect();
    let pkg_context = crate::build::artifacts::pkg_context::render(
        &crate::build::artifacts::pkg_context::PkgContextInput {
            module_name: &logical_name,
            project_dir: project,
            native_deps: &native,
            module_version: "1.0.0",
            ets_deps: &ets_deps,
            packages: &packages,
            app,
        },
    );
    let pc_path = out_dir.join("pkgContextInfo.json");
    std::fs::write(
        &pc_path,
        serde_json::to_string_pretty(&pkg_context).map_err(|e| e.to_string())?,
    )
    .map_err(|e| format!("write pkgContextInfo.json: {e}"))?;

    println!("✓ assembled at {}", out_dir.display());
    println!("  module.json         {}", final_module.display());
    println!("  pack.info           {}", pack_path.display());
    println!("  pkgContextInfo.json {}", pc_path.display());
    println!("  resources.index     {}", lifted_index.display());
    Ok(out_dir.to_path_buf())
}

/// Copy a directory tree flattening symlinks (the packaged module layout has
/// no symlinks; restool leaves some in its working area).
fn copy_dir_flat(src: &Path, dst: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dst).map_err(|e| format!("mkdir {}: {e}", dst.display()))?;
    for entry in std::fs::read_dir(src)
        .map_err(|e| format!("read {}: {e}", src.display()))?
        .flatten()
    {
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if from.is_dir() {
            copy_dir_flat(&from, &to)?;
        } else {
            std::fs::copy(&from, &to).map_err(|e| format!("copy {}: {e}", from.display()))?;
        }
    }
    Ok(())
}
