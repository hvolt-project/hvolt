// res_compile: drive restool from hvolt.toml config, standalone.
//
// restool is the SDK's native resource compiler (Mach-O/PE, no JRE). the reference feeds it a
// generated `resConfig.json`; this module builds that file from hvolt.toml and the module
// config, stages a resource tree with the generated `main_pages.json`, and runs restool.
//
// Verified against asr-ondevice: the ResourceTable.txt ids match the reference's output
// exactly (`app_name 0x01000000`, `EntryAbility_desc 0x01000005`, ...).

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{json, Value};

use crate::support::config::{AppConfig, ModuleConfig};

/// What restool needs: everything that names the resources and the module.
pub struct ResCompileInput<'a> {
    pub project_dir: &'a Path,
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
fn fill_route_map(
    stage: &Path,
    project_dir: &Path,
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
    let Some(entries) = json.get_mut("routerMap").and_then(|value| value.as_array_mut()) else {
        return Ok(());
    };
    for entry in entries.iter_mut() {
        let Some(source) = entry
            .get("pageSourceFile")
            .and_then(|value| value.as_str())
            .map(str::to_string)
        else {
            continue;
        };
        let owner = route_owner(project_dir, &source, module_name);
        let version = if owner == module_name { "" } else { "1.0.0" };
        let stem = source.trim_end_matches(".ets");
        entry["ohmurl"] = serde_json::json!(format!("@normalized:N&&&{owner}/{stem}&{version}"));
        entry["bundleName"] = serde_json::json!(bundle);
        entry["moduleName"] = serde_json::json!(module_name);
    }
    let text = serde_json::to_string_pretty(&json).map_err(|e| e.to_string())?;
    std::fs::write(&path, text).map_err(|e| format!("write {}: {e}", path.display()))?;
    Ok(())
}

/// Which module a `pageSourceFile` belongs to.
///
/// A module's `route_map.json` can name a page another module owns (`entry` lists
/// `xb_components`' `Xb_Studio`), and `ohmurl` has to name the owner. The source tree keeps
/// the flat layout (`<module>/components/…`), while `pageSourceFile` is written against the
/// standard one (`src/main/ets/components/…`), so the probe drops that prefix.
fn route_owner(project_dir: &Path, source: &str, module_name: &str) -> String {
    let relative = source.strip_prefix("src/main/ets/").unwrap_or(source);
    let Some(parent) = project_dir.parent() else {
        return module_name.to_string();
    };
    for entry in std::fs::read_dir(parent).into_iter().flatten().flatten() {
        let dir = entry.path();
        if !dir.is_dir() {
            continue;
        }
        if dir.join(relative).is_file() {
            return dir
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or(module_name)
                .to_string();
        }
    }
    module_name.to_string()
}

/// Stage the resource tree restool compiles: the project's `resources/` plus a
/// generated `profile/main_pages.json` (the reference injects it the same way).
pub fn stage_resources(input: &ResCompileInput<'_>, stage: &Path) -> Result<(), String> {
    let project_res = input.project_dir.join("resources");
    if !project_res.is_dir() {
        return Err(format!(
            "no resources/ directory at {}",
            project_res.display()
        ));
    }
    copy_dir_recursive(&project_res, stage)?;
    // restool assigns ids in input order, and hvigor feeds it two trees: the app-scope
    // resources first, then the module's. Reproducing that split keeps the ids aligned.
    let app_stage = stage.with_file_name("app-stage");
    split_app_resources(&project_res, input.app, stage, &app_stage)?;
    // The project may keep an app-scope tree of its own (`app-resources/`). restool numbers
    // the app scope before the module, so every media file in it has to reach `app-stage`;
    // leaving them in the module tree shifts every later id by their count (measured
    // 2026-11-07 on clashbox: 8 media stayed behind and every id came out 8 low).
    if let Some(app_res) = app_resource_dir(input.project_dir) {
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
    fill_route_map(
        stage,
        input.project_dir,
        &input.module.module.name,
        &input.app.app.bundle,
    )?;
    Ok(())
}

/// Move the app-level resources out of the module tree into their own directory.
///
/// The app scope carries what `[app]` itself references: the label string and the icon (with
/// the media that icon composes). Everything else belongs to the module.
fn split_app_resources(
    project_res: &Path,
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
    if let Ok(text) = std::fs::read_to_string(
        project_res
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
    let deps = discover_dependencies(input.project_dir, &input.module.module.name);
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
    if let Some(resource_str) = write_precompiled_strings(input.project_dir, &deps, input.out_dir) {
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
    deps: &[(String, std::path::PathBuf)],
    out_dir: &Path,
) -> Option<std::path::PathBuf> {
    // A byte-code HAR ships no `resources/`, only a table; walk the package store for those.
    let mut precompiled: Vec<PathBuf> = Vec::new();
    for store in [
        project_dir.join("oh_modules"),
        project_dir.join("oh_modules/.ohpm"),
    ] {
        collect_precompiled(&store, 0, &mut precompiled);
    }
    precompiled.sort();
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
    for (_, dir) in deps {
        // `<pkg>/src/main/resources` → the package root carries the table.
        let pkg_root = dir
            .parent()
            .and_then(|p| p.parent())
            .and_then(|p| p.parent())
            .unwrap_or(dir);
        let candidates = [
            pkg_root.join("ResourceTable.txt"),
            dir.parent().unwrap_or(dir).join("ResourceTable.txt"),
        ];
        let Some(text) = candidates
            .iter()
            .find_map(|p| std::fs::read_to_string(p).ok())
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
fn discover_dependencies(project_dir: &Path, own_module: &str) -> Vec<(String, PathBuf)> {
    let mut out: Vec<(String, PathBuf)> = Vec::new();
    // `project_dir` is either the module itself or the project root; both layouts occur, so
    // the module scan runs over the directory and over its parent.
    let mut roots: Vec<&Path> = vec![project_dir];
    if let Some(parent) = project_dir.parent() {
        roots.push(parent);
    }
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
    // Order matters: restool assigns ids in input order, so the reference feeds the
    // dependency graph depth-first from the module's declared dependencies. Sorting here
    // shifts every dependency-contributed id.
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
    if let Some(ordered) = dependency_order(project_dir, own_module) {
        let mut sorted: Vec<(String, PathBuf)> = Vec::new();
        for name in ordered {
            if let Some(pos) = out.iter().position(|(n, _)| *n == name) {
                sorted.push(out.remove(pos));
            }
        }
        sorted.extend(out);
        out = sorted;
    }
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

/// Depth-first order of the module's declared dependencies, as `oh-package.json5` lists them.
fn dependency_order(project_dir: &Path, own_module: &str) -> Option<Vec<String>> {
    let mut ordered: Vec<String> = Vec::new();
    for base in [
        project_dir.to_path_buf(),
        project_dir.parent()?.to_path_buf(),
    ] {
        let manifest = base.join("oh-package.json5");
        // A module directory usually has no manifest of its own; the declared order then
        // lives one level up. Missing the first candidate must fall through to the parent,
        // not abandon the whole walk — the difference is the dependency order, and restool
        // assigns ids in exactly that order (measured 2026-11-07: six dependency strings came
        // out rotated because the walk stopped at the module directory).
        let Ok(text) = std::fs::read_to_string(&manifest) else {
            continue;
        };
        let names = declared_dependencies(&text);
        if !names.is_empty() {
            walk_dependencies(&base, own_module, &names, &mut ordered);
            return Some(ordered);
        }
    }
    None
}

/// `dependencies` keys of a manifest, in declaration order (the file is JSON5-ish).
fn declared_dependencies(text: &str) -> Vec<String> {
    let Some(start) = text
        .find("\"dependencies\"")
        .or_else(|| text.find("dependencies"))
    else {
        return Vec::new();
    };
    let body = &text[start..];
    let Some(open) = body.find('{') else {
        return Vec::new();
    };
    let mut depth = 0usize;
    let mut end = body.len();
    for (i, ch) in body[open..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    end = open + i;
                    break;
                }
            }
            _ => {}
        }
    }
    let mut names = Vec::new();
    for line in body[open + 1..end].lines() {
        let line = line.trim().trim_end_matches(',');
        let Some((key, _)) = line.split_once(':') else {
            continue;
        };
        let key = key.trim().trim_matches('"').trim_matches('\'');
        if key.is_empty()
            || key.starts_with("//")
            || key.starts_with('*')
            || !key
                .chars()
                .next()
                .is_some_and(|c| c.is_alphanumeric() || c == '@')
        {
            continue;
        }
        names.push(key.to_string());
    }
    names
}

/// Walk `names` (and their transitive dependencies) depth-first, collecting package names.
fn walk_dependencies(
    base: &std::path::Path,
    own_module: &str,
    names: &[String],
    ordered: &mut Vec<String>,
) {
    for name in names {
        if name == own_module || ordered.iter().any(|n| n == name) {
            continue;
        }
        let local = base.join(name);
        let package_root = if local.is_dir() {
            Some(local)
        } else {
            find_package_dir(base, name)
        };
        let Some(package_root) = package_root else {
            continue;
        };
        ordered.push(name.clone());
        if let Ok(text) = std::fs::read_to_string(package_root.join("oh-package.json5")) {
            let nested = declared_dependencies(&text);
            if !nested.is_empty() {
                walk_dependencies(&package_root, own_module, &nested, ordered);
            }
        }
    }
}

/// `<base>/oh_modules/**/<name>` — the installed location of an ohpm package.
fn find_package_dir(base: &std::path::Path, name: &str) -> Option<PathBuf> {
    let flat = base.join("oh_modules").join(name);
    if flat.is_dir() {
        return Some(flat);
    }
    let store = base.join("oh_modules/.ohpm");
    let escaped = name.replace('/', "+");
    let entries = std::fs::read_dir(&store).ok()?.flatten();
    for entry in entries {
        let path = entry.path();
        let dir_name = path.file_name()?.to_string_lossy().to_string();
        if dir_name.starts_with(&format!("{escaped}@")) || dir_name == escaped {
            let nested = path.join("oh_modules").join(name);
            if nested.is_dir() {
                return Some(nested);
            }
        }
    }
    None
}

/// Module directories beside the entry module (`xb_components`, `proxy_core`, …).
fn scan_module_dirs(
    root: &Path,
    _project_dir: &Path,
    own_module: &str,
    out: &mut Vec<(String, PathBuf)>,
) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
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
pub fn compile(input: &ResCompileInput<'_>, stage: &Path) -> Result<PathBuf, String> {
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

    let restool = crate::sdk::manage::sdk_root()
        .and_then(|root| {
            let dir = root.join("default/openharmony/toolchains");
            crate::support::platform::exe_names("restool")
                .iter()
                .map(|n| dir.join(n))
                .find(|p| p.exists())
        })
        .ok_or_else(|| "restool not found in SDK toolchains".to_string())?;

    let output = Command::new(&restool)
        .arg("-l")
        .arg(&res_config)
        .output()
        .map_err(|e| format!("run restool: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        return Err(format!(
            "restool failed: {}{}",
            stdout.trim_end(),
            stderr.trim_end()
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

/// One-shot pipeline: stage the resources and run restool.
/// Returns the compiled ResourceTable.txt path.
pub fn run(
    project_dir: &Path,
    module: &ModuleConfig,
    app: &AppConfig,
    out_dir: &Path,
    build_mode: &str,
    facts: &crate::build::deps::sdk_facts::SdkFacts,
) -> Result<PathBuf, String> {
    std::fs::create_dir_all(out_dir).map_err(|e| format!("mkdir {}: {e}", out_dir.display()))?;
    let stage = out_dir.with_file_name("res-stage");
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
            has_route_map: project_dir
                .join("resources/base/profile/route_map.json")
                .is_file(),
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
        app,
        module,
        module_json_path: &module_json_path,
        out_dir,
    };
    // 1) stage: project resources/ + generated profile/main_pages.json
    stage_resources(&input, &stage)?;
    // 2) restool compile → resources.index, resources/, ResourceTable.*
    let table = compile(&input, &stage)?;

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
    use super::push_packages;
    use crate::support::config::{ModuleConfig, ModuleInfo};

    /// generate_main_pages (via stage) drops the .ets suffix and keeps order.
    #[test]
    fn main_pages_strips_ets_and_keeps_order() {
        let module = ModuleConfig {
            module: ModuleInfo {
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
}
