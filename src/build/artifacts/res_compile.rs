// res_compile: drive restool from hvolt.toml config, standalone.
//
// restool is the SDK's native resource compiler (Mach-O/PE, no JRE). the reference feeds it a
// generated `resConfig.json`; this module builds that file from hvolt.toml and the module
// config, stages a resource tree with the generated `main_pages.json`, and runs restool.
//
// Verified against asr-ondevice: the ResourceTable.txt ids match the reference's output
// exactly (`app_name 0x01000000`, `EntryAbility_desc 0x01000005`, ...).

use std::path::{Path, PathBuf};
#[cfg(test)]
use std::process::Command;

use serde_json::{json, Value};

use crate::support::config::{AppConfig, ModuleConfig};

/// What restool needs: everything that names the resources and the module.
pub struct ResCompileInput<'a> {
    pub project_dir: &'a Path,
    /// The module's own root: its `resources/` is what this compiles. A multi-module project
    /// keeps one per module (`<module>/resources`), so the staging root is not the project's.
    pub module_dir: &'a Path,
    pub app: &'a AppConfig,
    pub module: &'a ModuleConfig,
    /// The rendered module.json (process-profile shape) restool validates refs against.
    pub module_json_path: &'a Path,
    /// Where the compiled resources land: `resources.index`, `resources/`, `ResourceTable.*`.
    pub out_dir: &'a Path,
}

/// Fill the route records the installer resolves a `NavPathStack.pushPathByName` through.
///
/// The project's `route_map.json` carries only the build-time fields (`name`,
/// `pageSourceFile`, `buildFunction`, `data`); the packaged copy the reference ships is
/// filled with `ohmurl`, `bundleName` and `moduleName` per entry. A missing `ohmurl` makes
/// the push crash instead of resolving (`ImportConfigFromURL`/`ImportConfigFromFile` on
/// clashbox, measured 2026-11-08 — without it the runtime reported `can't find name in
/// config file` and created an empty node; with `routerMap` referenced but still unfilled it
/// took the process down).
///
/// `ohmurl` names the module that owns the page, which is the first segment of
/// `pageSourceFile`, and its version: the entry module's records carry none (a bare `&`),
/// a dependency module's carry its own.
fn write_route_map(
    stage: &Path,
    module_dir: &Path,
    module: &crate::support::config::ModuleInfo,
    bundle: &str,
) -> Result<(), String> {
    if module.router_map.is_empty() {
        // A project that has not migrated still ships its own `route_map.json`; the copy that
        // came with it carries no `ohmurl`, so fill it rather than leave it inert.
        return fill_staged_route_map(stage, module_dir, &module.name, bundle);
    }
    // Same resolver the lowering uses: one derivation, one failure mode. Two copies here
    // published a route the abc never registered.
    let resolved = crate::build::deps::routes::route_entries(Some(module), module_dir)?;
    let mut entries = Vec::new();
    for route in &resolved {
        let owner = route_owner(module_dir, &route.page, &module.name)?;
        let version = if owner == module.name { "" } else { "1.0.0" };
        let stem = route.page.strip_suffix(".ets").unwrap_or(&route.page);
        entries.push(serde_json::json!({
            "name": route.name,
            "pageSourceFile": format!("src/main/ets/{}", route.page),
            "buildFunction": route.build_function,
            "ohmurl": format!("@normalized:N&&&{owner}/src/main/ets/{stem}&{version}"),
            "bundleName": bundle,
            "moduleName": module.name,
        }));
    }
    let profile_dir = stage.join("base").join("profile");
    std::fs::create_dir_all(&profile_dir)
        .map_err(|e| format!("mkdir {}: {e}", profile_dir.display()))?;
    let text = serde_json::to_string_pretty(&serde_json::json!({ "routerMap": entries }))
        .map_err(|e| e.to_string())?;
    let path = profile_dir.join("route_map.json");
    std::fs::write(&path, text).map_err(|e| format!("write {}: {e}", path.display()))
}

/// Fill `ohmurl` / `bundleName` / `moduleName` into a `route_map.json` that came with the
/// project.
///
/// The installer resolves a `NavPathStack` push through `ohmurl`; a manifest copied verbatim
/// has none, so the push finds the name and no target. This is the path a project takes before
/// it is migrated, where the manifest is the source of truth.
fn fill_staged_route_map(
    stage: &Path,
    module_dir: &Path,
    module_name: &str,
    bundle: &str,
) -> Result<(), String> {
    let path = stage.join("base").join("profile").join("route_map.json");
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Ok(());
    };
    let Ok(mut json) = serde_json::from_str::<serde_json::Value>(&text) else {
        return Ok(());
    };
    let Some(entries) = json
        .get_mut("routerMap")
        .and_then(|value| value.as_array_mut())
    else {
        return Ok(());
    };
    for entry in entries.iter_mut() {
        let Some(page) = entry["pageSourceFile"]
            .as_str()
            .map(|source| source.trim_start_matches("src/main/ets/").to_string())
        else {
            continue;
        };
        let owner = route_owner(module_dir, &page, module_name)?;
        let version = if owner == module_name { "" } else { "1.0.0" };
        let stem = page.strip_suffix(".ets").unwrap_or(&page);
        entry["ohmurl"] = serde_json::json!(format!(
            "@normalized:N&&&{owner}/src/main/ets/{stem}&{version}"
        ));
        entry["bundleName"] = serde_json::json!(bundle);
        entry["moduleName"] = serde_json::json!(module_name);
    }
    let text = serde_json::to_string_pretty(&json).map_err(|e| e.to_string())?;
    std::fs::write(&path, text).map_err(|e| format!("write {}: {e}", path.display()))
}

/// Which module a route page belongs to.
///
/// A module's `[[module.router_map]]` can name a page another module owns (`entry` lists
/// `xb_components`' `Xb_Studio`), and the installer resolves the route through the owner's
/// `ohmurl`. The page path here is module-relative (`components/...`), so the probe tries the
/// flat layout (`<module>/components/...`) of each sibling module.
fn route_owner(module_dir: &Path, page: &str, own: &str) -> Result<String, String> {
    // The module that stages this page owns it unless a sibling also has a file at that
    // relative path.
    if module_dir.join(page).is_file() {
        return Ok(own.to_string());
    }
    let Some(workspace) = module_dir.parent() else {
        return Ok(own.to_string());
    };
    let mut candidates: Vec<std::path::PathBuf> = std::fs::read_dir(workspace)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|dir| dir.is_dir() && dir.join(page).is_file())
        .collect();
    candidates.sort();
    match candidates.len() {
        // No sibling has it: the page is this module's own file that is not staged yet.
        0 => Ok(own.to_string()),
        1 => {
            let cfg = crate::build::deps::routes::module_config(&candidates[0])
                .map_err(|error| format!("route `{page}`: {error}"))?;
            Ok(cfg.module.name)
        }
        // Two modules with the same relative page: the ohmurl would name one of them at
        // random. Refuse rather than publish a route that points at the wrong module.
        _ => Err(format!(
            "route `{page}` exists in more than one module ({}); a route must name a page \
             one module owns",
            candidates
                .iter()
                .map(|dir| dir.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

/// The widget manifest, from `[[module.forms]]`.
fn write_form_config(
    stage: &Path,
    module: &crate::support::config::ModuleInfo,
) -> Result<(), String> {
    if module.forms.is_empty() {
        return Ok(());
    }
    let forms: Vec<serde_json::Value> = module
        .forms
        .iter()
        .map(|form| {
            serde_json::json!({
                "name": form.name,
                "displayName": form.display_name,
                "description": form.description,
                "src": format!("./{}", form.src),
                "uiSyntax": form.ui_syntax,
                "window": form.window.as_ref().map(|window| serde_json::json!({
                    "designWidth": window.design_width,
                    "autoDesignWidth": window.auto_design_width,
                })),
                "colorMode": form.color_mode,
                "isDynamic": form.is_dynamic,
                "isDefault": form.is_default,
                "updateEnabled": form.update_enabled,
                "scheduledUpdateTime": form.scheduled_update_time,
                "updateDuration": form.update_duration,
                "defaultDimension": form.default_dimension,
                "supportDimensions": form.support_dimensions,
            })
        })
        .collect();
    write_profile(
        stage,
        "form_config.json",
        &serde_json::json!({ "forms": forms }),
    )
}

/// `backup_config.json`, from `[module.backup]`.
fn write_backup_config(
    stage: &Path,
    module: &crate::support::config::ModuleInfo,
) -> Result<(), String> {
    let Some(backup) = &module.backup else {
        return Ok(());
    };
    write_profile(
        stage,
        "backup_config.json",
        &serde_json::json!({ "allowToBackupRestore": backup.allow_to_backup_restore }),
    )
}

fn write_profile(stage: &Path, name: &str, value: &serde_json::Value) -> Result<(), String> {
    let profile_dir = stage.join("base").join("profile");
    std::fs::create_dir_all(&profile_dir)
        .map_err(|e| format!("mkdir {}: {e}", profile_dir.display()))?;
    let path = profile_dir.join(name);
    let text = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    std::fs::write(&path, text).map_err(|e| format!("write {}: {e}", path.display()))
}

/// Overlay the selected product's resource directories onto the staged tree.
///
/// The list is ordered: a directory declared later replaces what an earlier one wrote. An
/// element table (`*/element/*.json`) is merged by resource name, so a product can redefine one
/// string without dropping the table it inherits; every other file is replaced whole.
fn overlay_product_resources(input: &ResCompileInput<'_>, stage: &Path) -> Result<(), String> {
    let Some(product) = input.app.resolved.as_ref() else {
        return Ok(());
    };
    for directory in &product.resource_directories {
        let source = input.project_dir.join(directory);
        if !source.is_dir() {
            return Err(format!(
                "product `{}`: resource directory `{}` not found at {}",
                product.name,
                directory,
                source.display()
            ));
        }
        overlay_dir(&source, stage)?;
    }
    Ok(())
}

fn overlay_dir(source: &Path, dest: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dest).map_err(|e| format!("mkdir {}: {e}", dest.display()))?;
    for entry in std::fs::read_dir(source)
        .map_err(|e| format!("read {}: {e}", source.display()))?
        .flatten()
    {
        let from = entry.path();
        let to = dest.join(entry.file_name());
        if from.is_dir() {
            overlay_dir(&from, &to)?;
            continue;
        }
        let element_table = from.extension().is_some_and(|ext| ext == "json")
            && from
                .parent()
                .and_then(|parent| parent.file_name())
                .is_some_and(|name| name == "element");
        if element_table && to.is_file() {
            merge_element_table(&from, &to)?;
        } else {
            std::fs::copy(&from, &to)
                .map_err(|e| format!("copy {} → {}: {e}", from.display(), to.display()))?;
        }
    }
    Ok(())
}

/// Merge one element table over the inherited one: same `name` wins, other entries survive.
fn merge_element_table(from: &Path, to: &Path) -> Result<(), String> {
    let read = |path: &Path| -> Result<serde_json::Value, String> {
        let text =
            std::fs::read_to_string(path).map_err(|e| format!("read {}: {e}", path.display()))?;
        serde_json::from_str(&text).map_err(|e| format!("parse {}: {e}", path.display()))
    };
    let later = read(from)?;
    let earlier = read(to)?;
    let (Some(later), Some(earlier)) = (later.as_object(), earlier.as_object()) else {
        return std::fs::copy(from, to)
            .map(|_| ())
            .map_err(|e| format!("copy {}: {e}", from.display()));
    };
    let mut merged = earlier.clone();
    for (kind, value) in later {
        let Some(entries) = value.as_array() else {
            merged.insert(kind.clone(), value.clone());
            continue;
        };
        let redefined: Vec<&str> = entries
            .iter()
            .filter_map(|entry| entry.get("name").and_then(|name| name.as_str()))
            .collect();
        let mut table: Vec<serde_json::Value> = merged
            .get(kind)
            .and_then(|value| value.as_array())
            .map(|inherited| {
                inherited
                    .iter()
                    .filter(|entry| {
                        entry
                            .get("name")
                            .and_then(|name| name.as_str())
                            .is_none_or(|name| !redefined.contains(&name))
                    })
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();
        table.extend(entries.iter().cloned());
        merged.insert(kind.clone(), serde_json::Value::Array(table));
    }
    let text = serde_json::to_string_pretty(&serde_json::Value::Object(merged))
        .map_err(|e| e.to_string())?;
    std::fs::write(to, text).map_err(|e| format!("write {}: {e}", to.display()))
}

/// Stage the resource tree restool compiles: the project's `resources/` plus a
/// generated `profile/main_pages.json` (the reference injects it the same way).
pub(crate) fn stage_resources(input: &ResCompileInput<'_>, stage: &Path) -> Result<(), String> {
    // Two layouts are valid: a migrated project keeps resources beside its module
    // (`entry/resources/`), a scaffolded one keeps them at the project root
    // (`resources/`, which is what `hvolt init` writes). Prefer the module, fall back to the
    // root — checking only one of them broke `hvolt init` projects.
    let project_res = if input.module_dir.join("resources").is_dir() {
        input.module_dir.join("resources")
    } else {
        input.project_dir.join("resources")
    };
    if !project_res.is_dir() {
        return Err(format!(
            "no resources/ directory at {} or {}",
            input.module_dir.join("resources").display(),
            input.project_dir.join("resources").display()
        ));
    }
    copy_dir_recursive(&project_res, stage)?;
    // The selected product's directories are overlaid before the app scope is split out: an
    // icon or label a product redefines has to land in the same tree the split reads.
    overlay_product_resources(input, stage)?;
    // restool assigns ids in input order, and hvigor feeds it two trees: the app-scope
    // resources first, then the module's. Reproducing that split keeps the ids aligned.
    let app_stage = stage.with_file_name("app-stage");
    // Same rule as `stage`: rebuilt from the sources every run, so a resource the user
    // deleted cannot survive here and reappear in the HAP.
    if app_stage.exists() {
        std::fs::remove_dir_all(&app_stage)
            .map_err(|e| format!("clean {}: {e}", app_stage.display()))?;
    }
    split_app_resources(input.app, stage, &app_stage)?;
    // The project may keep an app-scope tree of its own (`app-resources/`). restool numbers
    // the app scope before the module, so every media file in it has to reach `app-stage`;
    // leaving them in the module tree shifts every later id by their count (measured
    // 2026-11-07 on clashbox: 8 media stayed behind and every id came out 8 low).
    if let Some(app_res) = app_resource_dir(input.project_dir) {
        // The app scope also carries the element tables `module.json` references
        // (`$string:app_name` is its label). Without them restool rejects the manifest with
        // `The resource reference '$string:app_name' is not defined` — the app-scope tree a
        // migrated project keeps is `app-resources/`, not the module's `resources/`.
        let app_base = app_res.join("base");
        if app_base.is_dir() {
            for entry in std::fs::read_dir(&app_base).into_iter().flatten().flatten() {
                let path = entry.path();
                if !path.is_dir() || entry.file_name() == "media" {
                    continue;
                }
                copy_dir_recursive(&path, &app_stage.join("base").join(entry.file_name()))?;
            }
        }
        let app_media = app_res.join("base/media");
        if app_media.is_dir() {
            copy_dir_recursive(&app_media, &app_stage.join("base/media"))?;
            // Move, don't copy: a file that stays in the module tree as well would be
            // counted twice and change the table's size.
            let module_media = stage.join("base/media");
            for entry in std::fs::read_dir(&app_media)
                .into_iter()
                .flatten()
                .flatten()
            {
                let staged = module_media.join(entry.file_name());
                if staged.is_file() {
                    std::fs::remove_file(&staged)
                        .map_err(|e| format!("remove {}: {e}", staged.display()))?;
                }
            }
        }
    }
    // main_pages.json: routes from the module config → resources/base/profile/.
    let profile_dir = stage.join("base").join("profile");
    std::fs::create_dir_all(&profile_dir)
        .map_err(|e| format!("mkdir {}: {e}", profile_dir.display()))?;
    let main_pages = crate::translator::generate_main_pages(input.module);
    std::fs::write(profile_dir.join("main_pages.json"), main_pages)
        .map_err(|e| format!("write main_pages.json: {e}"))?;
    write_route_map(
        stage,
        input.module_dir,
        &input.module.module,
        &input.app.app.bundle,
    )?;
    write_form_config(stage, &input.module.module)?;
    write_backup_config(stage, &input.module.module)?;
    Ok(())
}

/// Move the app-level resources out of the module tree into their own directory.
///
/// The app scope carries what `[app]` itself references: the label string and the icon (with
/// the media that icon composes). Everything else belongs to the module.
fn split_app_resources(
    app: &AppConfig,
    module_stage: &Path,
    app_stage: &Path,
) -> Result<(), String> {
    let icon = &app.app.icon;
    let media_name = icon
        .split(':')
        .next_back()
        .unwrap_or_default()
        .trim()
        .to_string();
    let mut moved_media: Vec<String> = Vec::new();
    if !media_name.is_empty() {
        moved_media.push(format!("{media_name}.png"));
        moved_media.push(format!("{media_name}.json"));
    }
    // A layered image names the media it composes; those move with it.
    // Read the staged tree, not the source: a product overlay may define the icon's
    // layered-image manifest without also shipping the module's copy.
    if let Ok(text) = std::fs::read_to_string(
        module_stage
            .join("base/media")
            .join(format!("{media_name}.json")),
    ) {
        for token in text.split("\"$media:") {
            if let Some(name) = token.split('"').next() {
                let name = name.trim();
                if !name.is_empty() && name != media_name {
                    moved_media.push(format!("{name}.png"));
                }
            }
        }
    }

    let app_element = app_stage.join("base").join("element");
    std::fs::create_dir_all(&app_element)
        .map_err(|e| format!("mkdir {}: {e}", app_element.display()))?;
    let app_media = app_stage.join("base").join("media");
    std::fs::create_dir_all(&app_media)
        .map_err(|e| format!("mkdir {}: {e}", app_media.display()))?;

    // Strings: the app label lives in the app scope, the rest stay with the module.
    let module_strings = module_stage.join("base/element/string.json");
    if let Ok(text) = std::fs::read_to_string(&module_strings) {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) {
            let mut app_entries = Vec::new();
            let mut module_entries = Vec::new();
            for entry in value
                .get("string")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default()
            {
                let name = entry
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();
                if name == "app_name" {
                    app_entries.push(entry);
                } else {
                    module_entries.push(entry);
                }
            }
            if !app_entries.is_empty() {
                let body = serde_json::json!({ "string": app_entries });
                std::fs::write(
                    app_element.join("string.json"),
                    serde_json::to_string_pretty(&body).map_err(|e| e.to_string())?,
                )
                .map_err(|e| format!("write app string.json: {e}"))?;
                let body = serde_json::json!({ "string": module_entries });
                std::fs::write(
                    &module_strings,
                    serde_json::to_string_pretty(&body).map_err(|e| e.to_string())?,
                )
                .map_err(|e| format!("rewrite module string.json: {e}"))?;
            }
        }
    }

    for file in moved_media {
        let from = module_stage.join("base/media").join(&file);
        if from.is_file() {
            let to = app_media.join(&file);
            std::fs::copy(&from, &to).map_err(|e| format!("copy {}: {e}", from.display()))?;
            std::fs::remove_file(&from).map_err(|e| format!("remove {}: {e}", from.display()))?;
        }
    }
    Ok(())
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dst).map_err(|e| format!("mkdir {}: {e}", dst.display()))?;
    for entry in std::fs::read_dir(src)
        .map_err(|e| format!("read {}: {e}", src.display()))?
        .flatten()
    {
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if from.is_dir() {
            // An empty directory is not a resource: restool reads `resources/<qualifier>/…`
            // and rejects a directory with nothing under it (`Invalid qualifier key 'media'`).
            // Coping it through fails a build that has nothing wrong with its resources.
            let empty = std::fs::read_dir(&from)
                .map(|mut entries| entries.next().is_none())
                .unwrap_or(false);
            if empty {
                continue;
            }
            copy_dir_recursive(&from, &to)?;
        } else {
            std::fs::copy(&from, &to).map_err(|e| format!("copy {}: {e}", from.display()))?;
        }
    }
    Ok(())
}

/// Write the resConfig.json restool consumes.
/// `stage` holds the compiled resource tree (project resources/ + generated
/// main_pages.json); restool reads moduleResources from there, not from the
/// project sources.
fn write_res_config(
    input: &ResCompileInput<'_>,
    res_config: &Path,
    stage: &Path,
) -> Result<(), String> {
    let ids_map = input.out_dir.join("ids_map");
    // System id definitions come with the (commercial) hms component. When it
    // is absent, restool accepts an empty-record placeholder — an OpenHarmony
    // app that only uses its own resources does not need system ids, and the
    // OpenHarmony component's own id_defined.json trips restool's duplicate
    // checks.
    let defined_sys_ids = if let Some(root) = crate::sdk::manage::sdk_root() {
        let hms = root.join("default/hms/toolchains/id_defined.json");
        if hms.exists() {
            hms
        } else {
            std::fs::create_dir_all(&ids_map).map_err(|e| format!("mkdir ids_map: {e}"))?;
            let placeholder = ids_map.join("sys_id_defined.json");
            std::fs::write(&placeholder, "{\"record\":[]}")
                .map_err(|e| format!("write sys_id_defined: {e}"))?;
            placeholder
        }
    } else {
        PathBuf::from("/nonexistent")
    };
    let mut cfg = serde_json::Map::new();
    cfg.insert("configPath".into(), json!(input.module_json_path));
    cfg.insert("packageName".into(), json!(input.app.app.bundle));
    cfg.insert("output".into(), json!(input.out_dir));
    // restool compiles the entry module *and* every dependency module into one table; the
    // names list has to agree with `dependencies` or the ids land under the wrong package.
    // The dependency order restool sees is the declaration order of `hvolt.toml`; no hvigor
    // manifest is read to derive it.
    let declared: Vec<String> = input.app.dependencies.keys().cloned().collect();
    // Every local package the project declares (`path = "deps/<scope>/<name>"`), in table
    // order: that is where a dependency's own resources and its precompiled table live.
    let package_roots = declared_package_roots(input.project_dir, input.app);
    let deps = discover_dependencies(
        input.project_dir,
        &input.module.module.name,
        &declared,
        &package_roots,
    );
    // `getRestoolModuleNames()` lists the project's own modules only; ohpm packages are
    // dependencies, not modules.
    let module_names = local_module_names(input.project_dir, &input.module.module.name);
    cfg.insert("moduleNames".into(), json!(module_names.join(",")));
    cfg.insert(
        "ResourceTable".into(),
        json!([
            input.out_dir.join("ResourceTable.h"),
            input.out_dir.join("ResourceTable.ts"),
        ]),
    );
    // `getAppResourceDir()`: the product's declared resource directory, else the app-scope
    // resources. The project may name that directory differently (`app-resources`), and a
    // missing app resource directory shifts every id in the table.
    // A hvolt project keeps one `resources/` tree; the app-level part of it is split into
    // `app-stage` by `stage_resources` so restool sees the same two inputs hvigor feeds it.
    let app_stage = input.out_dir.with_file_name("app-stage");
    if app_stage.is_dir() {
        cfg.insert("applicationResource".into(), json!(app_stage));
    } else if let Some(app_res) = app_resource_dir(input.project_dir) {
        cfg.insert("applicationResource".into(), json!(app_res));
    }
    // The staged tree carries the project resources *plus* what the build generates into
    // them (`main_pages.json`); pointing restool at the sources would drop those entries from
    // the index. Application resources are already split out, so their relative order no
    // longer depends on this directory being the source tree.
    let module_res = stage.to_path_buf();
    cfg.insert("moduleResources".into(), json!([module_res]));
    // A precompiled HAR ships its own `ResourceTable.txt`: its ids cannot be regenerated, so
    // the names have to exist in this table too. The reference materialises them into a
    // generated `resource_str` directory and passes it as the last dependency.
    let mut dep_dirs: Vec<std::path::PathBuf> = deps.iter().map(|(_, dir)| dir.clone()).collect();
    let mut precompiled_roots: Vec<PathBuf> = package_roots
        .iter()
        .map(|(_, package)| package.clone())
        .collect();
    precompiled_roots.extend(deps_package_roots(input.project_dir));
    if let Some(resource_str) =
        write_precompiled_strings(input.project_dir, &precompiled_roots, input.out_dir)
    {
        dep_dirs.push(resource_str);
    }
    cfg.insert("dependencies".into(), json!(dep_dirs));
    // `isEnabledIconCheck()` is on by default for a HarmonyOS HAP.
    cfg.insert("iconCheck".into(), json!(true));
    cfg.insert("ids".into(), json!(ids_map));
    cfg.insert("definedIds".into(), json!(ids_map.join("id_defined.json")));
    cfg.insert("definedSysIds".into(), json!(defined_sys_ids));
    let config = Value::Object(cfg);
    let text = serde_json::to_string_pretty(&config).map_err(|e| e.to_string())?;
    std::fs::write(res_config, text).map_err(|e| format!("write {}: {e}", res_config.display()))
}

/// Collect the string names a precompiled HAR declares and materialise them in one
/// directory, mirroring the reference's `resource_str` dependency.
fn write_precompiled_strings(
    project_dir: &Path,
    packages: &[PathBuf],
    out_dir: &Path,
) -> Option<std::path::PathBuf> {
    // A byte-code HAR ships no `resources/`, only a table. Packages linked from `deps/` are
    // read from their own roots; the old `oh_modules` store is still walked when it exists.
    let mut precompiled: Vec<PathBuf> = packages.to_vec();
    for store in [
        project_dir.join("oh_modules"),
        project_dir.join("oh_modules/.ohpm"),
    ] {
        collect_precompiled(&store, 0, &mut precompiled);
    }
    precompiled.sort();
    precompiled.dedup();
    let mut names: Vec<String> = Vec::new();
    for root in &precompiled {
        if let Ok(text) = std::fs::read_to_string(root.join("ResourceTable.txt")) {
            for line in text.lines() {
                let mut parts = line.split_whitespace();
                let (Some(kind), Some(name)) = (parts.next(), parts.next()) else {
                    continue;
                };
                if kind == "string" && !names.iter().any(|n| n == name) {
                    names.push(name.to_string());
                }
            }
        }
    }
    for package in packages {
        // `<pkg>/src/main/resources` → the package root carries the table.
        let candidates = [
            package.join("ResourceTable.txt"),
            package.join("src/main/resources/ResourceTable.txt"),
        ];
        let Some(text) = candidates
            .iter()
            .find_map(|path| std::fs::read_to_string(path).ok())
        else {
            continue;
        };
        for line in text.lines() {
            let mut parts = line.split_whitespace();
            let (Some(kind), Some(name)) = (parts.next(), parts.next()) else {
                continue;
            };
            if kind == "string" && !names.iter().any(|n| n == name) {
                names.push(name.to_string());
            }
        }
    }
    if names.is_empty() {
        return None;
    }
    let dir = out_dir.join("resource_str").join("base").join("element");
    std::fs::create_dir_all(&dir).ok()?;
    let entries: Vec<serde_json::Value> = names
        .iter()
        .map(|n| json!({ "name": n, "value": "" }))
        .collect();
    let body = serde_json::to_string_pretty(&json!({ "string": entries })).ok()?;
    std::fs::write(dir.join("string.json"), body).ok()?;
    Some(out_dir.join("resource_str").canonicalize().unwrap_or(dir))
}

/// The app-level resource directory: the product's declared one, else the app scope.
fn app_resource_dir(project_dir: &Path) -> Option<PathBuf> {
    for candidate in [
        project_dir.join("AppScope/resources"),
        project_dir.join("app-resources"),
        project_dir.join("app_resources"),
    ] {
        if candidate.is_dir() {
            return Some(candidate.canonicalize().unwrap_or(candidate));
        }
    }
    None
}

/// The project's own module names, entry first (the reference's `moduleNames`).
fn local_module_names(project_dir: &Path, own_module: &str) -> Vec<String> {
    let mut names = vec![own_module.to_string()];
    let Some(root) = project_dir.parent() else {
        return names;
    };
    // The parent scan sees the project root itself as an entry. A project root with its own
    // `resources/` therefore entered `moduleNames` as a module (measured on clashbox:
    // `entry,clashbox,proxy_core,xb_components`), and restool then placed `entry` second, so
    // every resource id started at `0x02000000` instead of `0x01000000`. Exclude by resolved
    // path, not by name: a sibling module may legitimately share a name with something else.
    let own_dir = project_dir
        .canonicalize()
        .unwrap_or_else(|_| project_dir.to_path_buf());
    let roots = [root, project_dir];
    for dir in roots {
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        let mut found: Vec<String> = entries
            .flatten()
            .filter_map(|e| {
                let p = e.path();
                if !p.is_dir() {
                    return None;
                }
                let name = p.file_name()?.to_string_lossy().to_string();
                if name.starts_with('.') || name == "oh_modules" || name == "build" {
                    return None;
                }
                if p.canonicalize().unwrap_or_else(|_| p.clone()) == own_dir {
                    return None;
                }
                let has_res = p.join("resources").is_dir() || p.join("src/main/resources").is_dir();
                if has_res && name != own_module {
                    Some(name)
                } else {
                    None
                }
            })
            .collect();
        found.sort();
        for name in found {
            if !names.contains(&name) {
                names.push(name);
            }
        }
    }
    names
}

/// The resource directories restool has to see besides the entry module's own.
///
/// The reference feeds `dependencies` a list of directories — one per HAR/HSP module and one
/// per ohpm dependency — and the ids for their resources only exist if they are in it.
/// Discovery walks the layout rather than a config table because the module list lives in
/// the caller, not here.
fn discover_dependencies(
    project_dir: &Path,
    own_module: &str,
    declared: &[String],
    package_roots: &[(String, PathBuf)],
) -> Vec<(String, PathBuf)> {
    let mut out: Vec<(String, PathBuf)> = Vec::new();
    // Declared local packages first: a `path` dependency names its directory exactly, so its
    // `resources/` (or `src/main/resources/`) needs no guessing. `deps/<scope>/<name>` is the
    // layout `hvolt dep sync` writes, and `oh_modules` no longer exists in such a project.
    for (name, package) in package_roots {
        if let Some(res) = package_resources(package) {
            out.push((name.clone(), res));
        }
    }
    // `project_dir` is either the module itself or the project root; both layouts occur, so
    // the module scan runs over the directory and over its parent.
    let mut roots: Vec<&Path> = vec![project_dir];
    if let Some(parent) = project_dir.parent() {
        roots.push(parent);
    }
    // Local packages under `deps/` that the table did not name (a module-level dependency, or
    // a directory left behind): still a dependency's resources, so still in `dependencies`.
    push_deps_packages(&mut out, project_dir);
    for root in roots {
        scan_module_dirs(root, project_dir, own_module, &mut out);
        push_packages(&mut out, root.join("oh_modules").join(".ohpm"));
        push_packages(&mut out, root.join("oh_modules"));
    }
    // restool resolves these against its own working directory; the reference feeds it
    // absolute paths, and a relative one silently drops the dependency module.
    for (_, dir) in out.iter_mut() {
        if let Ok(canonical) = dir.canonicalize() {
            *dir = canonical;
        }
    }
    // restool rejects a duplicated input path outright, so unique by directory before and
    // after the reorder (the package store yields the same package through two scans).
    let mut seen: Vec<PathBuf> = Vec::new();
    out.retain(|(_, dir)| {
        if seen.contains(dir) {
            false
        } else {
            seen.push(dir.clone());
            true
        }
    });
    // Order matters: restool assigns ids in input order. The order is the dependency table
    // in `hvolt.toml`, which is the only declaration the build reads.
    let mut sorted: Vec<(String, PathBuf)> = Vec::new();
    for name in declared {
        if let Some(pos) = out.iter().position(|(n, _)| n == name) {
            sorted.push(out.remove(pos));
        }
    }
    sorted.extend(out);
    out = sorted;
    let mut seen: Vec<PathBuf> = Vec::new();
    out.retain(|(_, dir)| {
        if seen.contains(dir) {
            false
        } else {
            seen.push(dir.clone());
            true
        }
    });
    out
}

/// Module directories beside the entry module (`xb_components`, `proxy_core`, …).
fn scan_module_dirs(
    root: &Path,
    project_dir: &Path,
    own_module: &str,
    out: &mut Vec<(String, PathBuf)>,
) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    // The parent scan sees the project root itself as a sibling entry. A project root with its
    // own `resources/` then entered `dependencies` as a module of its own (measured on
    // clashbox: `entry,proxy_core,xb_components` plus the root `clashbox`), and restool counted
    // one package too many. Exclude it by resolved path, not by name.
    let project_dir = project_dir
        .canonicalize()
        .unwrap_or_else(|_| project_dir.to_path_buf());
    let mut siblings: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
    siblings.sort();
    for dir in siblings {
        if !dir.is_dir() {
            continue;
        }
        let name = dir
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        if name.starts_with('.') || name == "oh_modules" || name == "build" || name == own_module {
            continue;
        }
        if dir.canonicalize().unwrap_or_else(|_| dir.clone()) == project_dir {
            continue;
        }
        let res = if dir.join("resources").is_dir() {
            dir.join("resources")
        } else if dir.join("src/main/resources").is_dir() {
            dir.join("src/main/resources")
        } else {
            continue;
        };
        out.push((name, res));
    }
}

/// The package's own resource directory, whichever layout it uses.
fn package_resources(package: &Path) -> Option<PathBuf> {
    [
        package.join("resources"),
        package.join("src/main/resources"),
    ]
    .into_iter()
    .find(|candidate| candidate.is_dir())
}

/// `[dependencies]` entries that point at a local package, in table order.
fn declared_package_roots(project_dir: &Path, app: &AppConfig) -> Vec<(String, PathBuf)> {
    app.dependencies
        .iter()
        .filter_map(|(name, dependency)| {
            dependency
                .path
                .as_deref()
                .map(|path| (name.clone(), project_dir.join(path)))
        })
        .collect()
}

/// Local packages under `deps/`: `deps/<name>` or `deps/<scope>/<name>`.
fn deps_package_roots(project_dir: &Path) -> Vec<PathBuf> {
    let deps_dir = project_dir.join("deps");
    let Ok(entries) = std::fs::read_dir(&deps_dir) else {
        return Vec::new();
    };
    let mut roots: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect();
    roots.sort();
    let mut packages = Vec::new();
    for root in roots {
        if package_resources(&root).is_some() || root.join("hvolt.toml").is_file() {
            packages.push(root);
            continue;
        }
        // A scope directory only groups packages (`deps/<scope>/<name>`).
        if let Ok(children) = std::fs::read_dir(&root) {
            let mut scoped: Vec<PathBuf> = children
                .flatten()
                .map(|entry| entry.path())
                .filter(|path| path.is_dir())
                .collect();
            scoped.sort();
            packages.extend(scoped);
        }
    }
    packages
}

/// Local packages under `deps/`, for a caller that needs their resource directories.
fn push_deps_packages(out: &mut Vec<(String, PathBuf)>, project_dir: &Path) {
    let deps_dir = project_dir.join("deps");
    for package in deps_package_roots(project_dir) {
        let Some(res) = package_resources(&package) else {
            continue;
        };
        let Some(name) = package
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
        else {
            continue;
        };
        // `deps/<scope>/<name>`: a dependency key that names a scope (`@acme/widget`).
        let scope = package
            .parent()
            .filter(|parent| *parent != deps_dir)
            .and_then(|parent| parent.file_name())
            .map(|scope| scope.to_string_lossy().to_string());
        match scope {
            Some(scope) => out.push((format!("@{scope}/{name}"), res)),
            None => out.push((name, res)),
        }
    }
}

/// ohpm dependencies: `<pkg>/src/main/resources` from both store layouts.
fn push_packages(out: &mut Vec<(String, PathBuf)>, base: PathBuf) {
    let Ok(entries) = std::fs::read_dir(&base) else {
        return;
    };
    {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            // Three store layouts: `<base>/<pkg>`, `<base>/<scope>/<pkg>` and
            // `<base>/<pkg>@<ver>/oh_modules/<pkg>`.
            let nested = path.join("oh_modules");
            let mut candidates = Vec::new();
            if nested.is_dir() {
                candidates = std::fs::read_dir(&nested)
                    .map(|it| it.flatten().map(|e| e.path()).collect::<Vec<_>>())
                    .unwrap_or_default();
            } else if path
                .file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with('@'))
            {
                candidates = std::fs::read_dir(&path)
                    .map(|it| it.flatten().map(|e| e.path()).collect::<Vec<_>>())
                    .unwrap_or_default();
            } else {
                candidates.push(path.clone());
            }
            // The package store nests a scoped package one level deeper
            // (`<pkg>@<ver>/oh_modules/@scope/<pkg>`), so a candidate whose own name starts
            // with `@` is a scope directory, not a package. The reference's `dependencies`
            // carries `@linke/ui-color-picker/src/main/resources` between `xb_components` and
            // `@mcui/mccharts` (its resConfig.json, measured 2026-11-06); leaving it out drops
            // that package's two string ids and shifts every later dependency-contributed id.
            for candidate in expand_scopes(candidates) {
                let res = candidate.join("src/main/resources");
                if res.is_dir() {
                    let name = candidate
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default();
                    if name.is_empty() {
                        continue;
                    }
                    let full = match candidate.parent().and_then(|p| p.file_name()) {
                        Some(parent) if parent != "oh_modules" => {
                            format!("{}/{}", parent.to_string_lossy(), name)
                        }
                        _ => name,
                    };
                    out.push((full, res));
                }
            }
        }
    }
}

/// Replace every scope directory with the packages it holds.
///
/// A package directory named `@scope` only ever appears as the parent of packages
/// (`oh_modules/@scope/pkg`); a package's own directory name never starts with `@`.
fn expand_scopes(candidates: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut expanded = Vec::new();
    for candidate in candidates {
        let scoped = candidate
            .file_name()
            .is_some_and(|n| n.to_string_lossy().starts_with('@'));
        if scoped {
            if let Ok(entries) = std::fs::read_dir(&candidate) {
                expanded.extend(entries.flatten().map(|e| e.path()));
            }
        } else {
            expanded.push(candidate);
        }
    }
    expanded
}

/// Package roots under `dir` that ship a `ResourceTable.txt`.
fn collect_precompiled(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    if depth > 6 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if path.join("ResourceTable.txt").is_file() {
            out.push(path.clone());
        }
        collect_precompiled(&path, depth + 1, out);
    }
}

/// A short, stable fingerprint of the restool input set.
fn config_stamp(text: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in text.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

/// Run restool with the staged resources. Returns the ResourceTable.txt path.
pub(crate) fn compile(input: &ResCompileInput<'_>, stage: &Path) -> Result<PathBuf, String> {
    // id_defined.json must exist with an empty record: restool treats a missing or
    // empty-object file as a config error, but accepts `{"record":[]}`.
    let ids_map = input.out_dir.join("ids_map");
    std::fs::create_dir_all(&ids_map).map_err(|e| format!("mkdir {}: {e}", ids_map.display()))?;
    let defined = ids_map.join("id_defined.json");
    if !defined.exists() {
        std::fs::write(&defined, "{\"record\":[]}")
            .map_err(|e| format!("write {}: {e}", defined.display()))?;
    }

    let res_config = input.out_dir.join("resConfig.json");
    write_res_config(input, &res_config, stage)?;
    // `ids_map` keeps ids stable across builds — the reference treats the config as a task
    // input, so a changed input set starts a fresh map. Reusing ours after the input set
    // changed pins the old ids and every `$r(…)` in the abc points at the wrong resource.
    let config_text = std::fs::read_to_string(&res_config).unwrap_or_default();
    let stamp = config_stamp(&config_text);
    let stamp_path = ids_map.join("config.stamp");
    let previous = std::fs::read_to_string(&stamp_path).unwrap_or_default();
    if previous.trim() != stamp {
        for entry in std::fs::read_dir(&ids_map).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.is_file() {
                let _ = std::fs::remove_file(&path);
            }
        }
        std::fs::write(&stamp_path, &stamp)
            .map_err(|e| format!("write {}: {e}", stamp_path.display()))?;
        let defined = ids_map.join("id_defined.json");
        if !defined.exists() {
            std::fs::write(&defined, "{\"record\":[]}")
                .map_err(|e| format!("write {}: {e}", defined.display()))?;
        }
    }

    // The index is written in-tree. `res_ids` reproduces restool's allocation (verified to zero
    // disagreement on a real project) and `res_index_build` + `res_index_write` reproduce the
    // file byte for byte (verified at 185751 bytes on the same project), so the SDK binary is
    // no longer on this path. The SDK tool stays reachable from `cfg(test)` only, which is how the
    // two products are compared when something on the device disagrees; a release build has no
    // path to it at all.
    #[cfg(test)]
    if std::env::var("HVOLT_RESTOOL").as_deref() == Ok("1") {
        return run_restool(&res_config, input);
    }
    let table = write_index_in_tree(input, &res_config, stage)?;
    Ok(table)
}

/// Run the SDK's restool, kept for comparison against the in-tree writer.
#[cfg(test)]
fn run_restool(res_config: &Path, input: &ResCompileInput<'_>) -> Result<PathBuf, String> {
    let restool = crate::sdk::manage::sdk_root()
        .and_then(|root| {
            let dir = crate::sdk::manage::component_root(&root, "toolchains");
            crate::support::platform::exe_names("restool")
                .iter()
                .map(|n| dir.join(n))
                .find(|p| p.exists())
        })
        .ok_or_else(|| "restool not found in SDK toolchains".to_string())?;
    let output = Command::new(&restool)
        .arg("-l")
        .arg(res_config)
        .output()
        .map_err(|e| format!("run restool: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "restool failed: {}{}",
            String::from_utf8_lossy(&output.stdout).trim_end(),
            String::from_utf8_lossy(&output.stderr).trim_end()
        ));
    }
    let table = input.out_dir.join("ResourceTable.txt");
    if !table.exists() {
        return Err(format!(
            "restool succeeded but no ResourceTable.txt at {}",
            table.display()
        ));
    }
    Ok(table)
}

/// Write `resources.index`, `resources/`, `ResourceTable.txt` and the id map without restool.
///
/// The staged trees and the ids restool would hand out are the whole input; everything else is
/// derived. `resConfig.json` is read back because it already names the three input lists in
/// restool's order (`applicationResource`, `moduleResources`, `dependencies`).
fn write_index_in_tree(
    input: &ResCompileInput<'_>,
    res_config: &Path,
    stage: &Path,
) -> Result<PathBuf, String> {
    let config: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(res_config).map_err(|e| format!("read resConfig.json: {e}"))?,
    )
    .map_err(|e| format!("parse resConfig.json: {e}"))?;

    let module_name = input.module.module.name.clone();
    let mut owned: Vec<(super::res_ids::GroupKind, String)> = Vec::new();
    if let Some(dir) = config.get("applicationResource").and_then(|v| v.as_str()) {
        owned.push((
            super::res_ids::GroupKind::ApplicationResource,
            dir.to_string(),
        ));
    }
    for dir in config
        .get("moduleResources")
        .and_then(|v| v.as_array())
        .into_iter()
        .flatten()
        .filter_map(|v| v.as_str())
    {
        owned.push((super::res_ids::GroupKind::ModuleResources, dir.to_string()));
    }
    for dir in config
        .get("dependencies")
        .and_then(|v| v.as_array())
        .into_iter()
        .flatten()
        .filter_map(|v| v.as_str())
    {
        owned.push((super::res_ids::GroupKind::Dependency, dir.to_string()));
    }

    // Module id: the position of this module in `moduleNames`, which restool builds from the
    // same list and uses as the high byte of every id.
    let module_names: Vec<String> = config
        .get("moduleNames")
        .and_then(|v| v.as_str())
        .map(|list| {
            list.split(',')
                .map(|name| name.trim().to_string())
                .collect()
        })
        .unwrap_or_default();
    // The high byte of every id is one-based (`0x01` for the first module), which is why the
    // position in `moduleNames` is shifted rather than used directly.
    let module_id = module_names
        .iter()
        .position(|name| *name == module_name)
        .map(|index| index as u32 + 1)
        .ok_or_else(|| format!("module {module_name} is missing from moduleNames"))?;

    let inputs: Vec<super::res_index_build::Input<'_>> = owned
        .iter()
        .map(|(kind, dir)| super::res_index_build::Input {
            kind: *kind,
            dir: Path::new(dir),
            // Every record names the module's own tree: restool copies dependency resources
            // into it before writing the index.
            sources: format!("{module_name}/resources"),
        })
        .collect();

    // ids handed out by an earlier build are kept: a changed id makes every `$r(…)` in the
    // already-compiled abc point at a different resource.
    let preset = read_preset(input.out_dir)?;
    let index = super::res_index_build::compile(RESTOOL_VERSION, module_id, &inputs, &preset)?;
    let bytes = super::res_index_write::write(&index);
    std::fs::write(input.out_dir.join("resources.index"), bytes)
        .map_err(|e| format!("write resources.index: {e}"))?;

    // The table and the id map come from the same assignment the index was built with.
    let mut entries: Vec<(super::res_index::ResourceType, String, u32)> = Vec::new();
    for config in &index.configs {
        for entry in &config.entries {
            entries.push((entry.kind, entry.name.clone(), entry.id));
        }
    }
    let mut unique: std::collections::BTreeMap<
        (u32, String),
        (super::res_index::ResourceType, String, u32),
    > = Default::default();
    for (kind, name, id) in entries {
        unique
            .entry((kind.code(), name.clone()))
            .or_insert((kind, name, id));
    }
    let rows: Vec<(super::res_index::ResourceType, String, u32)> = unique.into_values().collect();
    let table = input.out_dir.join("ResourceTable.txt");
    std::fs::write(&table, super::res_ids::render_table(&rows))
        .map_err(|e| format!("write ResourceTable.txt: {e}"))?;
    let ids_map = input.out_dir.join("ids_map");
    std::fs::create_dir_all(&ids_map).map_err(|e| format!("mkdir {}: {e}", ids_map.display()))?;
    std::fs::write(
        ids_map.join("id_defined.json"),
        super::res_ids::render_id_defined(&rows),
    )
    .map_err(|e| format!("write id_defined.json: {e}"))?;

    // The table and the id map are written from the index in memory; recomputing the
    // allocation from the same staged trees catches a write path that has drifted from the
    // allocator before the index reaches a device. `HVOLT_RES_ID_CHECK=0` skips the pass.
    if std::env::var("HVOLT_RES_ID_CHECK").as_deref() != Ok("0") {
        super::id_check::verify(input.out_dir, &module_name)?;
    }

    // `resources/` is package content: rawfile and profile files plus the media the media
    // records name. restool writes it; here it is copied from the staged tree.
    copy_staged_content(stage, &input.out_dir.join("resources"))?;
    Ok(table)
}

/// The version block restool stamps into `resources.index`. The device does not read it, but a
/// byte-identical file is what makes the replacement verifiable against the tool.
const RESTOOL_VERSION: &str = "Restool 6.1.0.003";

/// Ids an earlier build already handed out, as `(type code, name, id)`.
fn read_preset(out_dir: &Path) -> Result<std::collections::BTreeSet<(u32, String, u32)>, String> {
    let path = out_dir.join("ids_map").join("id_defined.json");
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Ok(Default::default());
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
        return Ok(Default::default());
    };
    let mut out = std::collections::BTreeSet::new();
    let Some(rows) = value.get("record").and_then(|v| v.as_array()) else {
        return Ok(out);
    };
    for row in rows {
        let Some(kind) = row.get("type").and_then(|v| v.as_str()) else {
            continue;
        };
        let Some(name) = row.get("name").and_then(|v| v.as_str()) else {
            continue;
        };
        // The id is written as a hex string (`"0x01000000"`), not a number.
        let Some(id) = row
            .get("id")
            .and_then(|v| v.as_str())
            .and_then(|text| u32::from_str_radix(text.trim_start_matches("0x"), 16).ok())
        else {
            continue;
        };
        if let Some(kind) = super::res_index_build::type_named(kind) {
            out.insert((kind.code(), name.to_string(), id));
        }
    }
    Ok(out)
}

/// Copy the media and profile files of every qualified tree into the packaged `resources/`.
fn copy_staged_content(stage: &Path, target: &Path) -> Result<(), String> {
    let Ok(entries) = std::fs::read_dir(stage) else {
        return Ok(());
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let qualifier = entry.file_name().to_string_lossy().to_string();
        for kind in ["media", "profile"] {
            let from = path.join(kind);
            if from.is_dir() {
                copy_dir_recursive(&from, &target.join(&qualifier).join(kind))?;
            }
        }
        // `rawfile` is packaged as the directory it is, at every qualifier that has one.
        let rawfile = path.join("rawfile");
        if rawfile.is_dir() {
            copy_dir_recursive(&rawfile, &target.join(&qualifier).join("rawfile"))?;
        }
    }
    Ok(())
}

/// One-shot pipeline: stage the resources and run restool.
/// Returns the compiled ResourceTable.txt path.
pub(crate) fn run(
    project_dir: &Path,
    module_dir: &Path,
    module: &ModuleConfig,
    app: &AppConfig,
    out_dir: &Path,
    build_mode: &str,
    facts: &crate::build::deps::sdk_facts::SdkFacts,
) -> Result<PathBuf, String> {
    std::fs::create_dir_all(out_dir).map_err(|e| format!("mkdir {}: {e}", out_dir.display()))?;
    let stage = out_dir.with_file_name("res-stage");
    // Rebuilt from the sources on every run: a resource the user deleted would otherwise
    // survive here and make restool fail on a directory that no longer exists upstream.
    if stage.exists() {
        std::fs::remove_dir_all(&stage).map_err(|e| format!("clean {}: {e}", stage.display()))?;
    }
    std::fs::create_dir_all(&stage).map_err(|e| format!("mkdir {}: {e}", stage.display()))?;

    // module.json is the config restool validates resource refs against. Empty
    // ResourceTable: restool fills the real ids into its own output copy.
    let ids = crate::build::compile::ets_resources::parse_table("");
    let manifest = crate::build::artifacts::module_json::render(
        &crate::build::artifacts::module_json::RenderInput {
            app,
            module,
            ids: &ids,
            build_mode,
            compile_sdk_version: &facts.version,
            api_release_type: &facts.release_type,
            virtual_machine: &facts.virtual_machine,
            has_insight_intent: crate::build::artifacts::insight_intent::collect(
                project_dir,
                &module.module.name,
            )
            .map(|entries| !entries.is_empty())
            .unwrap_or(false),
            // Derived, not copied: a migrated project carries the route names in
            // `hvolt.toml` and the manifest is generated, so the source file is not the
            // signal — the table is.
            has_route_map: !crate::build::deps::routes::route_entries(
                Some(&module.module),
                module_dir,
            )
            .map_err(|error| format!("module.json: {error}"))?
            .is_empty(),
        },
    );
    let module_json_path = out_dir.join("module.json");
    std::fs::write(
        &module_json_path,
        serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())?,
    )
    .map_err(|e| format!("write module.json: {e}"))?;

    let input = ResCompileInput {
        project_dir,
        module_dir,
        app,
        module,
        module_json_path: &module_json_path,
        out_dir,
    };
    // 1) stage: project resources/ + generated profile/main_pages.json
    stage_resources(&input, &stage)?;
    // 2) compile → resources.index, resources/, ResourceTable.*
    let table = compile(&input, &stage)?;

    // The manifest is rewritten now that the ids exist: `iconId`, `labelId` and the ability's
    // `descriptionId` are resource references, and a manifest without them is not what the
    // reference package ships. restool used to do this rewrite; the ids come from hvolt's own
    // table now, so it happens here.
    let ids = crate::build::compile::ets_resources::parse_table(
        &std::fs::read_to_string(&table).map_err(|e| format!("read table: {e}"))?,
    );
    let manifest = crate::build::artifacts::module_json::render(
        &crate::build::artifacts::module_json::RenderInput {
            app,
            module,
            ids: &ids,
            build_mode,
            compile_sdk_version: &facts.version,
            api_release_type: &facts.release_type,
            virtual_machine: &facts.virtual_machine,
            has_insight_intent: crate::build::artifacts::insight_intent::collect(
                project_dir,
                &module.module.name,
            )
            .map(|entries| !entries.is_empty())
            .unwrap_or(false),
            has_route_map: !crate::build::deps::routes::route_entries(
                Some(&module.module),
                module_dir,
            )
            .map_err(|error| format!("module.json: {error}"))?
            .is_empty(),
        },
    );
    std::fs::write(
        &module_json_path,
        serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())?,
    )
    .map_err(|e| format!("write module.json with ids: {e}"))?;

    // 3) the intent manifest is package content, not a resource: writing it into the restool
    //    input would add a `profile insight_intent` entry the reference package does not have.
    if let Some(manifest) = crate::build::artifacts::insight_intent::render(
        project_dir,
        &module.module.name,
        &app.app.bundle,
    )? {
        let profile = out_dir.join("resources").join("base").join("profile");
        std::fs::create_dir_all(&profile)
            .map_err(|e| format!("mkdir {}: {e}", profile.display()))?;
        let text = serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())?;
        std::fs::write(profile.join("insight_intent.json"), text)
            .map_err(|e| format!("write insight_intent.json: {e}"))?;
    }
    Ok(table)
}

#[cfg(test)]
mod tests {
    use super::{
        declared_package_roots, deps_package_roots, discover_dependencies, local_module_names,
        overlay_dir, overlay_product_resources, push_packages, write_precompiled_strings,
        ResCompileInput,
    };
    use crate::support::config::{Dependency, ModuleConfig, ModuleInfo, ResolvedProduct};

    /// generate_main_pages (via stage) drops the .ets suffix and keeps order.
    #[test]
    fn main_pages_strips_ets_and_keeps_order() {
        let module = ModuleConfig {
            module: ModuleInfo {
                router_map: Vec::new(),
                main: None,
                forms: Vec::new(),
                backup: None,
                src_entry: None,
                name: "src".into(),
                entry: "EntryAbility.ets".into(),
                device_types: vec!["default".into()],
                routes: vec!["pages/Index.ets".into(), "pages/About.ets".into()],
                ace_super_visual_path: None,
                skills: vec![],
                metadata: vec![],
                module_type: String::new(),
                abilities: vec![],
                permissions: vec![],
                obfuscation: false,
                cpp_flags: String::new(),
                native_arguments: String::new(),
                obfuscation_rules: String::new(),
                icon: String::new(),
                start_window_icon: String::new(),
                orientation: String::new(),
                entry_label: String::new(),
                entry_description: String::new(),
                delivery_with_install: None,
                installation_free: None,
                abi_filters: vec!["arm64-v8a".into()],
            },
            dependencies: std::collections::BTreeMap::new(),
        };
        let pages = crate::translator::generate_main_pages(&module);
        assert!(pages.contains(r#""pages/Index""#), "{pages}");
        assert!(pages.contains(r#""pages/About""#), "{pages}");
        assert!(!pages.contains(".ets"), "{pages}");
    }

    /// A store entry (`<pkg>@<ver>/oh_modules/@scope/<pkg>`) yields the scope directory as
    /// its candidate; discovery has to descend it, or the package's resources never reach
    /// `dependencies` and every later dependency-contributed id shifts.
    #[test]
    fn a_scoped_package_inside_the_store_resolves_to_its_resources() {
        let root = std::env::temp_dir().join(format!(
            "hvolt-res-scope-{}-{}",
            std::process::id(),
            "expand"
        ));
        let _ = std::fs::remove_dir_all(&root);
        let pkg = root
            .join("@linke+ui-color-picker@1.0.4")
            .join("oh_modules")
            .join("@linke")
            .join("ui-color-picker");
        std::fs::create_dir_all(pkg.join("src/main/resources")).unwrap();
        let mut out = Vec::new();
        push_packages(&mut out, root.clone());
        let _ = std::fs::remove_dir_all(&root);
        assert_eq!(out.len(), 1, "{out:?}");
        assert_eq!(out[0].0, "@linke/ui-color-picker");
        assert!(out[0].1.ends_with("ui-color-picker/src/main/resources"));
    }

    fn write(path: &std::path::Path, text: &str) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, text).unwrap();
    }

    /// Product directories overlay in declaration order: a resource named in a later directory
    /// replaces the earlier one, and entries only the earlier directory declares survive.
    #[test]
    fn later_product_directory_wins_per_resource_name() {
        let root = std::env::temp_dir().join(format!(
            "hvolt-res-overlay-{}-{}",
            std::process::id(),
            "order"
        ));
        let _ = std::fs::remove_dir_all(&root);
        write(
            &root.join("base/base/element/string.json"),
            r#"{"string":[{"name":"app_name","value":"base"},{"name":"home_text","value":"base"}]}"#,
        );
        write(
            &root.join("release/base/element/string.json"),
            r#"{"string":[{"name":"home_text","value":"release"}]}"#,
        );
        write(&root.join("release/base/media/mark.png"), "release");
        write(&root.join("base/base/media/mark.png"), "base");

        let stage = root.join("stage");
        overlay_dir(&root.join("base"), &stage).unwrap();
        overlay_dir(&root.join("release"), &stage).unwrap();

        let text = std::fs::read_to_string(stage.join("base/element/string.json")).unwrap();
        let table: serde_json::Value = serde_json::from_str(&text).unwrap();
        let entries = table["string"].as_array().unwrap();
        let value_of = |name: &str| {
            entries
                .iter()
                .find(|entry| entry["name"] == name)
                .and_then(|entry| entry["value"].as_str())
                .map(str::to_string)
        };
        assert_eq!(value_of("home_text").as_deref(), Some("release"));
        assert_eq!(value_of("app_name").as_deref(), Some("base"));
        // A non-element file is replaced whole, so the later directory wins too.
        assert_eq!(
            std::fs::read_to_string(stage.join("base/media/mark.png")).unwrap(),
            "release"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    /// A declared directory that is not on disk fails by name instead of silently building
    /// the default resources.
    #[test]
    fn missing_product_resource_directory_is_rejected() {
        let root = std::env::temp_dir().join(format!(
            "hvolt-res-overlay-{}-{}",
            std::process::id(),
            "missing"
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let mut app = crate::support::config::AppConfig::default_for("overlay");
        app.resolved = Some(ResolvedProduct {
            name: "release".into(),
            resource_directories: vec!["res/release".into()],
        });
        let module = ModuleConfig::default_entry();
        let module_json = root.join("module.json");
        let out_dir = root.join("out");
        let input = ResCompileInput {
            project_dir: &root,
            module_dir: &root,
            app: &app,
            module: &module,
            module_json_path: &module_json,
            out_dir: &out_dir,
        };
        let error = overlay_product_resources(&input, &root.join("stage"))
            .expect_err("a missing overlay directory must be reported");
        assert!(error.contains("release"), "{error}");
        assert!(error.contains("res/release"), "{error}");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The project root sits in its parent directory, so the parent scan used to return it as a
    /// module of its own (measured on clashbox: a root with `resources/` made restool number
    /// `entry` second and shifted every resource id). Real siblings must survive.
    #[test]
    fn project_root_is_not_a_module_when_scanning_the_parent() {
        let root = std::env::temp_dir().join(format!(
            "hvolt-res-modnames-{}-{}",
            std::process::id(),
            "root"
        ));
        let _ = std::fs::remove_dir_all(&root);
        let project = root.join("proj");
        std::fs::create_dir_all(project.join("resources/base")).unwrap();
        std::fs::create_dir_all(root.join("proxy_core/src/main/resources")).unwrap();
        std::fs::create_dir_all(root.join("xb_components/src/main/resources")).unwrap();
        let names = local_module_names(&project, "entry");
        assert_eq!(names, ["entry", "proxy_core", "xb_components"], "{names:?}");
        assert!(!names.iter().any(|name| name == "proj"));
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Identity, not the directory name, decides: an alias pointing at the project root is the
    /// project root, while a module directory is kept under its own name.
    #[cfg(unix)]
    #[test]
    fn project_root_aliases_are_excluded_by_identity() {
        let root = std::env::temp_dir().join(format!(
            "hvolt-res-modnames-{}-{}",
            std::process::id(),
            "alias"
        ));
        let _ = std::fs::remove_dir_all(&root);
        let project = root.join("proj");
        std::fs::create_dir_all(project.join("src/main/resources")).unwrap();
        std::fs::create_dir_all(root.join("lib/src/main/resources")).unwrap();
        std::os::unix::fs::symlink(&project, root.join("proj-alias")).unwrap();
        let names = local_module_names(&project, "entry");
        assert_eq!(names, ["entry", "lib"], "{names:?}");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// `deps/<scope>/<name>` is the layout `hvolt dep sync` writes: the declared package's
    /// resources must reach restool, in the dependency table's order.
    #[test]
    fn dependency_resources_are_scanned_from_the_deps_layout() {
        let root = std::env::temp_dir().join(format!(
            "hvolt-res-deps-{}-{}",
            std::process::id(),
            "resources"
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("deps/acme/widget/src/main/resources/base")).unwrap();
        std::fs::create_dir_all(root.join("entry/resources/base")).unwrap();
        let mut app = crate::support::config::AppConfig::default_for("deps");
        app.dependencies.insert(
            "@acme/widget".into(),
            Dependency {
                path: Some("deps/acme/widget".into()),
                version: Some("1.0.0".into()),
                ..Dependency::default()
            },
        );
        let declared: Vec<String> = app.dependencies.keys().cloned().collect();
        let roots = declared_package_roots(&root, &app);
        let deps = discover_dependencies(&root, "entry", &declared, &roots);
        let widget = root.join("deps/acme/widget/src/main/resources");
        let widget = widget.canonicalize().unwrap_or(widget);
        let hits = deps.iter().filter(|(_, dir)| dir == &widget).count();
        assert_eq!(hits, 1, "the declared package contributes once: {deps:?}");
        assert_eq!(deps[0].0, "@acme/widget", "declared order comes first");
        let name = deps[deps.iter().position(|(_, dir)| dir == &widget).unwrap()]
            .0
            .clone();
        assert_eq!(name, "@acme/widget");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// A precompiled package under `deps/` contributes its string names through the generated
    /// `resource_str` directory — the same shape the reference feeds restool.
    #[test]
    fn precompiled_tables_under_deps_contribute_their_strings() {
        let root = std::env::temp_dir().join(format!(
            "hvolt-res-deps-{}-{}",
            std::process::id(),
            "precompiled"
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("deps/acme/widget")).unwrap();
        std::fs::write(
            root.join("deps/acme/widget/ResourceTable.txt"),
            "string widget_label 0x02000000\ncolor widget_color 0x02000001\nfloat widget_size 0x02000002\n",
        )
        .unwrap();
        let packages = deps_package_roots(&root);
        let out_dir = root.join("out");
        let resource_str = write_precompiled_strings(&root, &packages, &out_dir)
            .expect("a precompiled table must materialise resource_str");
        let table = std::fs::read_to_string(resource_str.join("base/element/string.json")).unwrap();
        assert!(table.contains("widget_label"), "{table}");
        assert!(!table.contains("widget_color"), "only strings are carried");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The project root sits in its parent directory, so the parent scan must not list it as a
    /// dependency module of its own (measured on clashbox: `moduleNames` and `dependencies`
    /// both carried the root `clashbox`).
    #[test]
    fn project_root_is_not_a_dependency_module() {
        let parent =
            std::env::temp_dir().join(format!("hvolt-res-deps-{}-{}", std::process::id(), "root"));
        let _ = std::fs::remove_dir_all(&parent);
        let root = parent.join("proj");
        std::fs::create_dir_all(root.join("resources/base")).unwrap();
        std::fs::create_dir_all(parent.join("lib/resources/base")).unwrap();
        let deps = discover_dependencies(&root, "entry", &[], &[]);
        let names: Vec<&str> = deps.iter().map(|(name, _)| name.as_str()).collect();
        assert_eq!(names, ["lib"], "{names:?}");
        let _ = std::fs::remove_dir_all(&parent);
    }
}
