//! ets-assemble: compile an `.ets` source tree into a merged `modules.abc`.
//!
//! Pages (`@Component` files) go through the full transpile pass; every other
//! file (EntryAbility, workers, …) gets its kit imports rewritten to the
//! `@ohos:` form es2abc expects and is otherwise copied verbatim. The mirror
//! mirror tree lands under `<ts_root>/<module>/<rel>`, so a record name is
//! `<module>.<path>`.

use std::path::{Path, PathBuf};

use oxc_allocator::Allocator;
use oxc_ast::ast::{TSEnumDeclaration, TSInterfaceDeclaration, TSTypeAliasDeclaration};
use oxc_parser::Parser;
use oxc_span::SourceType;

use crate::build::lower::ets_transpile::TranspileOptions;
use crate::build::stage::process_import::load_kit_configs;

/// Resolve the SDK's `es2abc` launcher (platform-aware: `build-win` vs
/// `build-mac`, `.exe` on Windows). The native (standalone) build chain and the
/// `assemble-ets` CLI both use this.
pub fn sdk_es2abc() -> Result<PathBuf, String> {
    crate::sdk::manage::sdk_root()
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
        .ok_or_else(|| "es2abc not found in SDK ets-loader".to_string())
}

/// Resolve the SDK's `kit_configs` directory (the ets-loader kit table es2abc
/// imports by `@ohos:`). DevEco-style roots carry `ets/...` directly; hvolt
/// downloads land under `openharmony/ets/...`.
/// Every `kit_configs` directory in the SDK, not just the first one found.
///
/// An SDK ships two: the OpenHarmony one and the HMS one. Some kits exist in only one of
/// them (`@kit.DeskTopExtensionKit` is HMS-only), so picking a single directory silently
/// drops those kits and leaves their imports unresolvable on the device.
pub fn sdk_kit_configs() -> Result<Vec<PathBuf>, String> {
    let sdk = crate::sdk::manage::sdk_root().ok_or_else(|| "no SDK root resolved".to_string())?;
    let mut dirs: Vec<PathBuf> = Vec::new();
    for entry in std::fs::read_dir(&sdk)
        .map_err(|e| format!("read sdk root {}: {e}", sdk.display()))?
        .flatten()
    {
        let root = entry.path();
        for candidate in [
            root.join("ets/build-tools/ets-loader/kit_configs"),
            root.join("openharmony/ets/build-tools/ets-loader/kit_configs"),
            root.join("hms/ets/build-tools/ets-loader/kit_configs"),
        ] {
            if candidate.is_dir() && !dirs.contains(&candidate) {
                dirs.push(candidate);
            }
        }
    }
    if dirs.is_empty() {
        return Err("kit_configs not found under SDK root".to_string());
    }
    dirs.sort();
    Ok(dirs)
}

/// Run the two-stage pass: transpile/rewrite into `ts_root`, then merge via
/// es2abc. Returns the path of the produced abc.
/// One module staged into the merged abc: the ETS tree that supplies it and the entry
/// declaration the device resolves against.
pub struct StageModule<'a> {
    pub name: &'a str,
    pub dir: &'a Path,
    /// The module-relative source the device addresses the module by (`EntryAbility.ets`).
    /// It is one of `entries`; the merged abc's loadable entry record is named after it.
    pub entry_file: &'a str,
    /// Module-relative compile roots: the module's pages, its abilities and its own entry.
    /// The merged abc carries these sources and everything they import. The reference builds
    /// the same set from `loader.json`'s `compileEntry` (pages, router-map targets) plus
    /// module.json's ability `srcEntry`, and hands it to rollup as the input graph.
    ///
    /// Empty = compile every source under `dir`. That is the rule for an HSP/HAR module: the
    /// reference puts its whole `ets/` tree into `compileEntry`
    /// (`generate-loader-json.js`: `allowToCollectEtsFolder`).
    pub entries: &'a [String],
    /// The module is a HAR (`hvolt.toml` `[module] type = "har"`).
    ///
    /// Reaches the lowering as `projectConfig.compileHar`, which decides one generated value:
    /// a `NavDestination`'s page path is `""` while a HAR compiles
    /// (`process_component_build.js::navigationOrNavDestinationCreateContent`).
    pub compile_har: bool,
    /// The `main` the module's `hvolt.toml` declares (`Index.ets` → `Index`).
    ///
    /// It is the name other modules address this one by. `None` falls back to the module's
    /// own manifest on disk, which is how a project that has not migrated still resolves.
    pub main: Option<&'a str>,
}

/// Everything a merge needs besides the staged sources themselves.
pub struct AssembleOptions<'a> {
    pub bundle: &'a str,
    pub version: &'a str,
    /// The app's `versionCode` (`AppFields::version_code`, derived from `version` when the
    /// project does not pin one): the `VERSION_CODE` the generated profile exports.
    pub version_code: u32,
    /// app.json5 `bundleType`: `"app"` or `"atomicService"` (`BUNDLE_TYPE`).
    pub bundle_type: &'a str,
    /// The product being built (`build-profile.json5` `products[].name`); also the directory
    /// the generated profile sits under (`<module>/build/<product>/generated/…`).
    pub product: &'a str,
    /// The module target the profile is generated for (`products[].targets[].name`, `default`).
    pub target: &'a str,
    /// hvigor's `getBuildModeName()`: `"debug"` or `"release"` (`BUILD_MODE_NAME`).
    pub build_mode: &'a str,
    /// app.json5 `buildVersion`; `None` when the app does not declare one — hvigor then leaves
    /// `BUILD_VERSION` out of the generated profile entirely.
    pub build_version: Option<&'a str>,
    pub kit_configs_dir: &'a [std::path::PathBuf],
    pub es2abc: &'a Path,
    pub compile_context_info: Option<&'a Path>,
    pub packages: &'a [crate::build::deps::npm::Package],
    pub target_api_version: u32,
    /// Debug build: the compiler records named lexical environments and debug info.
    pub debug: bool,
}

pub fn assemble(
    modules: &[StageModule<'_>],
    ts_root: &Path,
    out_abc: &Path,
    opts: &AssembleOptions<'_>,
) -> Result<std::path::PathBuf, String> {
    let AssembleOptions {
        bundle,
        version,
        version_code,
        bundle_type,
        product,
        target,
        build_mode,
        build_version,
        kit_configs_dir,
        es2abc,
        compile_context_info,
        packages,
        target_api_version,
        debug,
    } = *opts;
    let kits = load_kit_configs(kit_configs_dir).unwrap_or_default();
    // `$r('app.<kind>.<name>')` has to become the object literal the runtime expects: left
    // as a call it resolves against the system table only, and an app resource comes back as
    // an invalid id (`getStringSync` → 401).
    // `out_abc` is `<out>/ets/modules.abc`; the resource stage lands in `<out>/res`.
    let resource_table = out_abc
        .parent()
        .and_then(|ets| ets.parent())
        .map(|out| out.join("res/ResourceTable.txt"))
        .and_then(|path| std::fs::read_to_string(path).ok())
        .map(|text| crate::build::compile::ets_resources::parse_table(&text))
        .unwrap_or_default();
    // The module the device addresses without a version: the entry module.
    let entry_module = modules.first().map(|m| m.name).unwrap_or("");
    // The same module is what every route registration this task writes names
    // (`process_ui_syntax.js::routerBundleOrModule` reads `projectConfig.moduleName`), whatever
    // module a source file belongs to.
    crate::build::compile::abc_compile::set_assemble_module(entry_module);
    // Sibling modules an import may name directly (`from 'proxy_core'`).
    let members: Vec<String> = modules.iter().map(|m| m.name.to_string()).collect();
    let mut staged: Vec<(&StageModule<'_>, Vec<std::path::PathBuf>, StagingPaths)> = Vec::new();
    for m in modules {
        let (mut files, staged_at) = if m.entries.is_empty() {
            let mut all = Vec::new();
            collect_ets(m.dir, &mut all)?;
            (all, StagingPaths::new())
        } else {
            source_closure(m.dir, m.entries)?
        };
        files.sort();
        files.dedup();
        if files.is_empty() {
            return Err(format!("{}: no .ets files found", m.dir.display()));
        }
        if std::env::var("HVOLT_TRACE_WRITE").is_ok() {
            eprintln!(
                "[stage] name={} dir={} files={}",
                m.name,
                m.dir.display(),
                files.len()
            );
        }
        staged.push((m, files, staged_at));
    }
    let all_files: Vec<std::path::PathBuf> = staged
        .iter()
        .flat_map(|(_, files, _)| files.iter().cloned())
        .collect();
    // The generated build profile. hvigor's `CreateBuildProfile` task writes one per module
    // target (`<module>/build/<product>/generated/profile/<target>/BuildProfile.ets`), but the
    // ArkTS compiler only pulls in what the module's import graph reaches: the reference
    // package carries a single record, the entry module's. Every staged file here is compiled,
    // so the file is written only for a module whose lowered sources import it — decided while
    // the lowered text is at hand, in the staging loop below.
    let profile_record = build_profile_record(product, target);
    let mut profile_needed = vec![false; modules.len()];

    // A dependency module is imported through its package root (`from 'proxy_core'`), which
    // resolves to `Index`: aggregate every source file of that module into it, entry
    // module excepted (its entry is an ability, not a barrel).
    // The barrel is written after the source pass, because its own imports need the same
    // url rewriting the sources get: `export * from './Profile'` is a module request, and a
    // bare `./Profile` in the abc names nothing.
    let mut barrels: Vec<(&str, String)> = Vec::new();
    for (m, files, _) in &staged {
        if std::ptr::eq(*m, &modules[0]) {
            continue;
        }
        // A module that ships its own package index keeps it: the index may alias exports
        // (`export { CrashUtils as Xb_CrashUtils }`), which a generated barrel cannot.
        if files
            .iter()
            .any(|f| f.file_stem().is_some_and(|stem| stem == "Index"))
        {
            continue;
        }
        let mut body = String::new();
        for f in files {
            let Ok(rel) = f.strip_prefix(m.dir) else {
                continue;
            };
            let rel = rel.with_extension("").to_string_lossy().replace('\\', "/");
            if rel == "Index" {
                continue;
            }
            body.push_str(&format!("export * from './{rel}';\n"));
        }
        if !body.is_empty() {
            barrels.push((m.name, body));
        }
    }

    // Builder names are file-local declarations, but a builder defined in one file is
    // called from another; collect the whole project's names first.
    let mut project_builders = project_names(&all_files);
    for name in sdk_component_names(kit_configs_dir) {
        if !project_builders.contains(&name) {
            project_builders.push(name);
        }
    }
    // The entry name each module's own manifest declares (`Index` for a component library,
    // the fallback when there is no manifest). A member module is addressed through it, and
    // its package entry is the staged file at `<module>/<entry>`.
    let module_entries: std::collections::BTreeMap<String, String> = modules
        .iter()
        .map(|m| {
            (
                m.name.to_string(),
                m.main
                    .map(|main| {
                        let name = main.rsplit('/').next().unwrap_or(main);
                        name.trim_end_matches(".ets").to_string()
                    })
                    .or_else(|| {
                        crate::build::compile::abc_compile::declared_entry(
                            crate::build::compile::abc_compile::module_root(m.dir),
                        )
                    })
                    .unwrap_or_else(|| {
                        crate::build::compile::abc_compile::DEFAULT_MODULE_ENTRY.to_string()
                    }),
            )
        })
        .collect();
    crate::build::compile::abc_compile::set_module_entries(module_entries);
    // The entry module's own entry record, which the device looks up without a version.
    let entry_record_addr = modules
        .first()
        .map(|m| {
            let entry = m
                .entry_file
                .trim_start_matches("./")
                .trim_end_matches(".ets");
            crate::build::compile::abc_compile::module_path(m.name, entry)
        })
        .unwrap_or_default();
    // Every source is read before any is emitted: a call site has to know whether the name
    // it calls is a user component, and under which protocol.
    let mut component_index: crate::build::compile::component_index::ComponentMap =
        std::collections::BTreeMap::new();
    let mut builder_names: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    // A component library declares its components in its own sources, and those declarations
    // belong to the same table: a package file calls its siblings, and a project file calls the
    // package's components, so both need the names before either is emitted. The set of files
    // is the one the package source root contains — the same walk `stage_package` uses — and
    // the criterion for an entry is only that the file declares `@Component`: nothing here
    // names a package or a component.
    for package in packages
        .iter()
        .filter(|package| package.kind == crate::build::deps::npm::Kind::Arkts)
    {
        let mut sources: Vec<std::path::PathBuf> = Vec::new();
        collect_package_sources(&package.source_root, &mut sources)?;
        for source in sources {
            let Ok(text) = std::fs::read_to_string(&source) else {
                continue;
            };
            let key = source_key(source.strip_prefix(&package.source_root).unwrap_or(&source));
            let declarations = crate::build::compile::component_index::scan_declarations(
                &package.alias,
                &key,
                &text,
            );
            crate::build::compile::component_index::record(
                &mut component_index,
                &package.alias,
                &key,
                declarations,
            );
        }
    }
    // The project's own declarations are read second so that a project component wins a name
    // a dependency package happens to reuse.
    for (staged_module, files, _) in &staged {
        for f in files {
            let Ok(text) = std::fs::read_to_string(f) else {
                continue;
            };
            let key = source_key(f.strip_prefix(staged_module.dir).unwrap_or(f));
            let declarations = crate::build::compile::component_index::scan_declarations(
                staged_module.name,
                &key,
                &text,
            );
            crate::build::compile::component_index::record(
                &mut component_index,
                staged_module.name,
                &key,
                declarations,
            );
            for name in crate::build::compile::component_index::scan_builder_functions(&text) {
                builder_names.insert(name);
            }
        }
    }
    // A declaration file is still a declaration: a module that ships only `.d.ets` names its
    // components there, and nothing compiles it, so neither scan above can see them. The
    // reference reads the imported file's `@Component` / `@CustomDialog` declarations before it
    // decides a call site (`process_import.js` feeds them into
    // `componentCollection.customComponents`), which is why `SegmentButton` — `@kit.ArkUI`
    // reaching `@ohos.arkui.advanced.SegmentButton.d.ets` — and `CustomColorPickerDialog` — a
    // byte-code HAR's `.d.ets`, re-exported by its `Index.d.ets` — construct as children. A name
    // that never reaches the table resolves to `None`, and the call site lowers as a built-in
    // (`X.create(…)` / `X.pop()`) instead of the child frame the reference writes.
    //
    // Only what the sources import is read, and the owner is the request they wrote
    // (`@kit.ArkUI`, `@linke/ui-color-picker`): `component_index::resolve` follows the import
    // table, whose first candidate owner is that request itself. The declared text goes through
    // the same `preprocess` + oxc pass the index runs on a source, so a `.d.ets` needs no reader
    // of its own.
    let api_dirs = sdk_api_dirs(kit_configs_dir);
    let mut declarations: std::collections::BTreeSet<(String, std::path::PathBuf)> =
        std::collections::BTreeSet::new();
    let owners = staged
        .iter()
        .map(|(module, _, _)| module.name.to_string())
        .chain(
            packages
                .iter()
                .filter(|package| package.kind == crate::build::deps::npm::Kind::Arkts)
                .map(|package| package.alias.clone()),
        );
    for owner in owners {
        for (local, request) in crate::build::compile::component_index::imports_of(&owner) {
            for path in declaration_files_for_import(&request, &local, &api_dirs, packages) {
                declarations.insert((request.clone(), path));
            }
        }
    }
    for (owner, path) in declarations {
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let declarations =
            crate::build::compile::component_index::scan_declarations(&owner, "", &text);
        crate::build::compile::component_index::record(
            &mut component_index,
            &owner,
            "",
            declarations,
        );
    }
    if std::env::var_os("HVOLT_DEBUG_INDEX").is_some() {
        eprintln!(
            "[index] {} components, Xb_CircleButton={:?}",
            component_index.len(),
            component_index
                .get(&(
                    "entry".to_string(),
                    String::new(),
                    "Xb_CircleButton".to_string(),
                ))
                .map(|m| m.state_vars.clone())
        );
    }
    crate::build::compile::component_index::set(component_index);
    crate::build::compile::component_index::set_builders(builder_names);

    // `const enum` members are erased at their use sites, and the use can sit in a file that
    // does not declare the enum. Read the whole project's tables before any file is lowered,
    // so a page sees an enum a sibling module declares (`fixtures/const-enum-import`: the
    // declaration lives in `common/Shared.ets`, the use in the page). Same shape as the
    // builder-name pass above, and the same reason: a file cannot answer it alone.
    let const_enum_sources: Vec<String> = all_files
        .iter()
        .filter_map(|file| std::fs::read_to_string(file).ok())
        .collect();
    crate::build::lower::enum_init::set_const_enums(
        &crate::build::lower::enum_init::collect_const_enums(&const_enum_sources),
    );
    // Whether a state field's declared type is a value or an object decides the backstore class,
    // and the type name may be declared in a module the field's file does not contain
    // (`fixtures/observed-property-kind-import`). Read the project's exported type names in the
    // same pass, into their own table: this one answers "what kind of type is `X`", the one
    // above answers "what value does `A.B` render as".
    // The SDK declares its own enums in `*.d.ts` (`FontWeight`, `TextAlign`, …), and a field may
    // name one of them. They are appended after the project's own declarations, so a project type
    // of the same name still wins.
    let mut project_types = crate::build::stage::process_component_class::collect_type_declarations(
        &const_enum_sources,
    );
    // The packages the modules import keep their declarations next to the staged sources
    // (`<dir>/oh_modules/<pkg>`), and a field's type may name one (`ColorPickerType` from
    // `@linke/ui-color-picker` in `Xb_SettingItem.ets`). They are appended after the project's
    // own declarations and before the SDK's: the project installed this version of the package,
    // so it is nearer to the code than anything the SDK carries.
    for (name, decl) in crate::build::stage::process_component_class::collect_type_declarations(
        &package_declaration_sources(modules, &const_enum_sources),
    ) {
        if !project_types.iter().any(|(known, _)| *known == name) {
            project_types.push((name, decl));
        }
    }
    for (name, decl) in crate::build::stage::process_component_class::collect_sdk_enums(
        &sdk_declaration_sources(kit_configs_dir),
    ) {
        if !project_types.iter().any(|(known, _)| *known == name) {
            project_types.push((name, decl));
        }
    }
    crate::build::stage::process_component_class::set_project_types(&project_types);

    // Staged files whose source declares the shared-module directive. The flag is a column of
    // the filesInfo table the compiler reads, so it is collected from the sources while the
    // source path and the staged path are both at hand.
    let mut shared: std::collections::BTreeSet<std::path::PathBuf> =
        std::collections::BTreeSet::new();
    let mut any_page = false;
    // The source map is written after every file is staged: the map describes positions in the
    // final text, after the import/resource rewrites moved lines around.
    let mut map_entries: Vec<crate::build::compile::ets_source_map::MapEntry> = Vec::new();
    for (module_index, (m, files, staged_at)) in staged.iter().enumerate() {
        let module = m.name;
        let ets_dir = m.dir;
        let _ = (bundle, m.entry_file);
        for f in files {
            let source_rel = f
                .strip_prefix(ets_dir)
                .map_err(|_| format!("strip prefix {}", f.display()))?;
            // A root that only resolved by file name is staged at the path its declaration
            // names (`srcEntry`), which is the path the device loads the record by.
            let rel = staged_at
                .get(f)
                .map(|path| path.as_path())
                .unwrap_or(source_rel);
            // The staged tree keeps the module root followed by the source path; the addressable
            // record name adds the module source root in one place (`abc_compile::module_path`).
            let dest = ts_root.join(module).join(rel).with_extension("ts");
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| format!("mkdir {}: {e}", parent.display()))?;
            }
            // The reference names the file it is lowering once, before it picks a route
            // (`process_ui_syntax.resourceFileName`), and every `{ page, line, col }` object it
            // writes reads that name back. The staging loop is the same boundary here: without
            // it, a component file that is not a page keeps the coordinate of whichever file
            // this thread lowered last.
            crate::build::ets_compile::set_source_page(&source_coordinate(
                &format!("{module}/src/main/ets"),
                rel,
            ));
            // A call site resolves against the module this file belongs to. The index keeps
            // `(module, name)` keys, so every lookup the lowering makes — the child-component
            // check, the dialog builder rewrite, the class-side protocol — has to run inside
            // this scope; the file's own path does not carry the module.
            let previous_source = crate::build::compile::component_index::current_source();
            crate::build::compile::component_index::set_current_source(
                module,
                &source_key(source_rel),
            );
            let src =
                std::fs::read_to_string(f).map_err(|e| format!("read {}: {e}", f.display()))?;
            if is_shared_module(&src) {
                shared.insert(dest.clone());
            }
            // `@CustomDialog` structs are components too: leaving them on the plain path emits
            // their decorators (`@State`, `@Prop`) and never generates the view class.
            let is_component = crate::build::lex::has_decorator(
                &src,
                &["Component", "ComponentV2", "Entry", "CustomDialog"],
            );
            let content = if is_component {
                any_page = true;
                let page = rel.with_extension("").to_string_lossy().replace('\\', "/");
                let opts = TranspileOptions {
                    kit_config_dirs: kit_configs_dir,
                    project_builders: &project_builders,
                    route: if bundle.is_empty() || module.is_empty() {
                        None
                    } else {
                        Some(crate::build::lower::ets_transpile::RouteInfo {
                            bundle,
                            module,
                            page: &page,
                            compile_har: m.compile_har,
                        })
                    },
                };
                crate::build::lower::ets_transpile::transpile(&src, &opts)
                    .map_err(|e| format!("{}: {e}", f.display()))?
            } else {
                // Every plain module goes through the AST. The printed text is layout-equivalent
                // and keeps each import/export as a node, so a later pass cannot drop one; the
                // line-based rewriter stays only as the fallback for a source the parser
                // rejects after preprocessing.
                crate::build::lower::ets_plain::lower(&src, &kits)
                    .map_err(|e| format!("{}: {e}", f.display()))?
            };
            // A `declare enum` used to be folded into its member literal here, and the import
            // deleted with it. `declares_as_type` classifies an enum as a value now, so the
            // binding is read, the import stays and the reference keeps the member access —
            // which is what the reference build emits (103 `KeyCode.*` references on clashbox,
            // measured 2026-11-06). The pass that did the folding is gone with that premise.
            // The module's import set and its requests are settled together: which bindings the
            // file reads decides the erasure and the `import type` split, and a statement that
            // keeps a value binding is the one whose request resolves onto a record url. The
            // pass also keeps the two `type` provenances apart — see
            // `ets_transform::settle_system_imports` for the system half the lowering already
            // decided.
            let content = {
                let base = ets_dir;
                // The type-only names each sibling module declares, computed once per file.
                let member_declarations: Vec<(String, String)> = modules
                    .iter()
                    .map(|m| {
                        (
                            m.name.to_string(),
                            crate::build::deps::npm::type_declarations_in(m.dir),
                        )
                    })
                    .collect();
                let file_dir = rel.parent().unwrap_or_else(|| std::path::Path::new(""));
                let resolve = |spec: &str| -> Option<String> {
                    // A package import resolves to the declarations the package ships: a
                    // byte-code HAR has no record for its types.
                    // A normalized url names its package in its first segment(s).
                    let url_path = spec
                        .strip_prefix("@normalized:N&&&")
                        .and_then(|rest| rest.split('&').next());
                    if let Some(path) = url_path {
                        if let Some((_, declarations)) = member_declarations
                            .iter()
                            .find(|(name, _)| path == name || path.starts_with(&format!("{name}/")))
                        {
                            return Some(declarations.clone());
                        }
                        if let Some(package) = packages
                            .iter()
                            .find(|p| path == p.alias || path.starts_with(&format!("{}/", p.alias)))
                        {
                            return Some(crate::build::deps::npm::type_declarations(package));
                        }
                    }
                    if let Some(package) = packages
                        .iter()
                        .find(|p| spec == p.alias || spec.starts_with(&format!("{}/", p.alias)))
                    {
                        return Some(crate::build::deps::npm::type_declarations(package));
                    }
                    // A sibling module's package index re-exports types as well; the same
                    // rule decides whether the name has a runtime export behind it.
                    if let Some((_, declarations)) = member_declarations
                        .iter()
                        .find(|(name, _)| spec == name || spec.starts_with(&format!("{name}/")))
                    {
                        return Some(declarations.clone());
                    }
                    let mut target = base.join(file_dir);
                    for seg in spec.split('/') {
                        match seg {
                            "." | "" => {}
                            ".." => {
                                target.pop();
                            }
                            other => target.push(other),
                        }
                    }
                    let stem = target.with_extension("");
                    let resolved = ["ets", "ts"]
                        .iter()
                        .find_map(|ext| std::fs::read_to_string(stem.with_extension(ext)).ok());
                    if resolved.is_some() {
                        return resolved;
                    }
                    // A sibling-module import written against the source layout climbs out of
                    // the module and lands on that module's package index; its names are the
                    // union of everything the module declares.
                    if spec == "./Index" || spec.ends_with("/Index") {
                        return member_declarations
                            .iter()
                            .find(|(name, _)| name == module)
                            .map(|(_, declarations)| declarations.clone());
                    }
                    None
                };
                let rewritten = crate::build::ets_transform::rewrite_imports(
                    &content,
                    Some(&src),
                    &|request| {
                        // The module's own version rule, not the app version: the entry
                        // module's urls carry none (`abc_compile::record_version`).
                        let module_version = crate::build::compile::abc_compile::record_version(
                            module,
                            entry_module,
                        );
                        rewrite_spec(
                            request,
                            file_dir,
                            module,
                            module_version,
                            packages,
                            &members,
                            entry_module,
                            &profile_record,
                        )
                    },
                    &|request, name| {
                        resolve(request).is_some_and(|target| declares_as_type(&target, name))
                    },
                );
                match rewritten {
                    Some(rewritten) => rewritten,
                    None => {
                        if crate::build::ets_transform::parse_diagnostics_enabled() {
                            eprintln!("import rewrite skipped: {module} does not parse");
                        }
                        content
                    }
                }
            };
            // The flat layout stages a module's files at the module root, where the reference
            // roots them at `src/main/ets`; only the package entry sits at the module root on
            // both sides. A relative request the import pass left as written is type-only by
            // then, so the compiler erases it and the text can take the shape the reference's
            // source has: `xb_components/Index.ts` writes
            // `./src/main/ets/controllers/ControllerEventHandler`, because its `Index.ets`
            // lives at the module root while the target lives under the source root.
            let content = if module != entry_module
                && crate::build::compile::abc_compile::module_root(ets_dir) == ets_dir
                && rel.file_stem().and_then(|stem| stem.to_str())
                    == Some(crate::build::compile::abc_compile::module_entry(module))
            {
                crate::build::ets_transform::reroot_entry_requests(&content).unwrap_or(content)
            } else {
                content
            };
            // A file whose every import was type-only is still a program: an empty file is
            // skipped by the compiler while filesInfo still lists it, and the merge then
            // reports a program count mismatch.
            let content = if content.trim().is_empty() {
                "export {};\n".to_string()
            } else {
                content
            };
            // Relative requests resolve against the file's real location, which is also where
            // its targets are staged; only this file's own record moves. The module loop above
            // already did that for this file, so the call is gone from here: the barrel loop
            // below is the remaining caller.
            let content = rewrite_dynamic_package_imports(&content, packages);
            // `$r('app.<kind>.<name>')` becomes the literal the runtime expects; a call left
            // in place only resolves against the system table, so an app resource comes back
            // as an invalid id (`getStringSync` → 401 at startup).

            let content = rewrite_dialog_builders(&content, &src);
            // The literal records the module the package is built for, not the module that
            // owns the source file: the loader gets one `moduleName` for the whole CompileArkTS
            // task (`projectConfig.moduleName`, read by `addBundleAndModuleParam` in
            // `lib/process_ui_syntax.js`), so a resource used by `xb_components` and packed
            // into `entry` still records `entry`.
            let (content, missing) = crate::build::compile::ets_resources::rewrite(
                &content,
                &resource_table,
                bundle,
                entry_module,
            );
            if !missing.is_empty() && std::env::var_os("HVOLT_DEBUG_RES").is_some() {
                eprintln!("[res] {}: 未解析 {}", dest.display(), missing.len());
            }
            if crate::build::ets_transform::has_runtime_code(&content) == Some(true) {
                // The reference erases a module that declares nothing but types: it has no
                // record in the abc and no source map (`fixtures/import-type-audit`).
                continue;
            }
            // A module whose compiled text imports the build-profile record needs that file to
            // exist; one that does not must not get it, because everything staged is compiled.
            if !profile_needed[module_index] && imports_build_profile(&content, &profile_record) {
                profile_needed[module_index] = true;
            }
            map_entries.push(source_map_entry(
                module,
                m.entry_file,
                rel,
                &src,
                &content,
                is_component,
                &project_builders,
            ));
            if std::env::var("HVOLT_TRACE_WRITE").is_ok()
                && dest.to_string_lossy().contains("breakpoint/BreakPoint.ts")
            {
                eprintln!("[src] {} bytes={}", f.display(), src.len());
                let head: String = content.lines().take(3).collect::<Vec<_>>().join(" | ");
                eprintln!(
                    "[write] {} kind={} head={}",
                    dest.display(),
                    if is_component { "component" } else { "plain" },
                    head
                );
            }
            std::fs::write(&dest, content).map_err(|e| format!("write {}: {e}", dest.display()))?;
            crate::build::compile::component_index::restore_source(previous_source);
        }
    }
    // The profile module itself, at the path `rewrite_spec` sends `import … from 'BuildProfile'`
    // to. It is written after the source pass because only the compiled text says whether a
    // module reaches it; the compiler picks it up like any other staged record.
    for (m, needed) in modules.iter().zip(&profile_needed) {
        if !*needed {
            continue;
        }
        let path = ts_root
            .join(m.name)
            .join(&profile_record)
            .with_extension("ts");
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("mkdir {}: {e}", parent.display()))?;
        }
        let body = render_build_profile(&build_profile_fields(
            bundle,
            bundle_type,
            version_code,
            version,
            target,
            product,
            build_mode,
            debug,
            build_version,
        ));
        std::fs::write(&path, body).map_err(|e| format!("write {}: {e}", path.display()))?;
    }
    for (module, body) in &barrels {
        let content = rewrite_relative_imports(
            body,
            module,
            entry_module,
            std::path::Path::new("Index.ts"),
            packages,
            &members,
            &[],
            &profile_record,
        );
        let path = ts_root.join(module).join("Index.ts");
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("mkdir {}: {e}", parent.display()))?;
        }
        std::fs::write(&path, content).map_err(|e| format!("write Index: {e}"))?;
    }
    write_source_maps(out_abc, modules, version, &map_entries)?;
    if !any_page {
        return Err(format!(
            "{}: no @Component page found",
            modules[0].dir.display()
        ));
    }
    // Third-party packages are compiled into the same abc: the device resolves
    // `import { parse } from 'yaml'` through the package's own records. A byte-code HAR
    // has no source to stage; its abc is merged instead.
    // Only the packages the sources actually import are compiled: a declared but unused
    // dependency would otherwise pull its whole tree into the abc for nothing.
    let used = used_packages(ts_root, packages)?;
    let mut abc_inputs: Vec<std::path::PathBuf> = Vec::new();
    for package in packages.iter().filter(|p| used.contains(&p.name)) {
        if package.kind == crate::build::deps::npm::Kind::ByteCode {
            abc_inputs.push(crate::build::deps::npm::abc_path(package));
            continue;
        }
        let wanted = imported_package_paths(ts_root, packages, package)?;
        if std::env::var("HVOLT_TRACE_PKG").is_ok() {
            eprintln!(
                "[pkg] {} wanted={} {:?}",
                package.name,
                wanted.len(),
                wanted.iter().take(4).collect::<Vec<_>>()
            );
        }
        stage_package(
            package,
            ts_root,
            kit_configs_dir,
            &project_builders,
            &kits,
            &wanted,
        )?;
    }
    crate::build::compile::abc_compile::merge(&crate::build::compile::abc_compile::MergeInput {
        ts_root,
        es2abc,
        out_abc,
        file_threads: 8,
        target_api_version,
        debug,
        version,
        entry_module,
        entry_record: Some(entry_record_addr.as_str()),
        compile_context_info,
        npm: packages,
        abc_inputs: &abc_inputs,
        shared: &shared,
    })
}

/// Whether a source file declares the shared-module directive.
///
/// The rule is the reference's `collectSharedModule` (`check_shared_module.js`): the first
/// statement that is not an import must be the string literal `"use shared"`. In the reference
/// the result reaches `sharedModuleSet`, which `module_mode.js` writes into filesInfo and which
/// also switches that module's compile type; the emitted code keeps the statement as a leading
/// directive.
pub(crate) fn is_shared_module(source: &str) -> bool {
    const DIRECTIVE: &str = "use shared";
    let tokens = crate::build::lex::tokenize(source);
    let mut index = 0usize;
    while index < tokens.len() {
        let token = &tokens[index];
        match token.kind {
            crate::build::lex::Kind::Comment => index += 1,
            // An empty statement carries no value: `import 'x';;` still opens the file.
            crate::build::lex::Kind::Punct if token.is_punct(';') => index += 1,
            // An import is skipped whole: it ends at its module specifier, the first string
            // literal of the statement (`import 'x'`, `import { a } from 'x'`,
            // `import type { a } from 'x'`).
            crate::build::lex::Kind::Ident if token.text == "import" => {
                index += 1;
                while index < tokens.len() && tokens[index].kind != crate::build::lex::Kind::Str {
                    index += 1;
                }
                index += 1;
            }
            crate::build::lex::Kind::Str => {
                return token.text.trim_matches(|c| c == '"' || c == '\'') == DIRECTIVE;
            }
            _ => return false,
        }
    }
    false
}

/// Rewrite relative import specifiers to the record names the merged abc addresses them
/// by: `./ClashViewModel` from `<module>/EntryAbility.ts` becomes the normalized url
/// `@normalized:N&&&<module>/ClashViewModel&`.
// The import-rewrite parameters are all distinct pieces of module context; grouping them
// into a struct would only move the list.
#[allow(clippy::too_many_arguments)]
fn rewrite_relative_imports(
    content: &str,
    module: &str,
    entry_module: &str,
    rel: &std::path::Path,
    packages: &[crate::build::deps::npm::Package],
    members: &[String],
    inferred_types: &[String],
    profile_record: &str,
) -> String {
    // A module's own imports resolve to its own records, so they carry that module's version
    // rule: none for the entry module, its version for every other module.
    let version = crate::build::compile::abc_compile::record_version(module, entry_module);
    let dir = rel.parent().unwrap_or_else(|| std::path::Path::new(""));
    // The specifiers come from the shared scanner, so `transform` is not read as `from`, a
    // string or comment is never mistaken for an import, and the multi-line binding list is
    // covered as well as the one-line form.
    let mut edits: Vec<(usize, usize, String)> = Vec::new();
    let Some(specifiers) = crate::build::ets_transform::module_specifiers(content) else {
        // The lowering emits valid TypeScript, so a file that does not parse here is a defect
        // in this pipeline rather than an input to guess at: report it and leave the text as
        // it stands.
        if crate::build::ets_transform::parse_diagnostics_enabled() {
            eprintln!("relative-import rewrite skipped: {module} does not parse");
        }
        return content.to_string();
    };
    for specifier in specifiers {
        // A type-only statement the type pass inferred keeps the path as written; one the
        // source wrote explicitly is normalized like any other request.
        if specifier.type_only && inferred_types.iter().any(|r| r == &specifier.request) {
            continue;
        }
        let Some(replacement) = rewrite_spec(
            &specifier.request,
            dir,
            module,
            version,
            packages,
            members,
            entry_module,
            profile_record,
        ) else {
            continue;
        };
        let quote = &content[specifier.start..specifier.start + 1];
        edits.push((
            specifier.start,
            specifier.end,
            format!("{quote}{replacement}{quote}"),
        ));
    }
    apply_replacements(content, &edits)
}

/// `src` with each `(start, end, replacement)` edit applied in order; overlapping edits are
/// dropped rather than applied twice.
pub(crate) fn apply_replacements(src: &str, edits: &[(usize, usize, String)]) -> String {
    apply_replacements_tracked(src, edits).0
}

/// As [`apply_replacements`], reporting where the text grew.
///
/// Each entry is `(offset in the result, characters added)`: an offset measured on the result
/// is that many columns too far right once the entry that precedes it on the same line has been
/// passed. The child-block rewrite is the one caller that needs it — the parser reads the
/// rewritten text, and the positions it reports have to be the source's.
pub(crate) fn apply_replacements_tracked(
    src: &str,
    edits: &[(usize, usize, String)],
) -> (String, Vec<(usize, usize)>) {
    let mut sorted: Vec<&(usize, usize, String)> = edits.iter().collect();
    sorted.sort_by_key(|edit| edit.0);
    let mut out = String::with_capacity(src.len());
    let mut grown = Vec::new();
    let mut cursor = 0usize;
    for (start, end, text) in sorted {
        if *start < cursor {
            continue;
        }
        out.push_str(&src[cursor..*start]);
        if text.len() > end.saturating_sub(*start) {
            grown.push((out.len(), text.len() - (end - start)));
        }
        out.push_str(text);
        cursor = *end;
    }
    out.push_str(&src[cursor..]);
    (out, grown)
}

/// The index key of one source: its path inside its owner, extension dropped, `/`-separated.
///
/// It is the path the declaration scan and the lowering both compute, so a call site resolves
/// the file its own import names (`'./Tooltip'` from `pageView/Charts` is `pageView/Tooltip`).
fn source_key(path: &std::path::Path) -> String {
    path.with_extension("").to_string_lossy().replace('\\', "/")
}

/// The coordinate a file names itself by inside a `{ page, line, col }` object.
///
/// `process_component_member.js::componentParamRowAndColumn` writes
/// `path.relative(process.cwd(), process_ui_syntax.resourceFileName)` — the file being lowered,
/// as a project-relative path with its source extension. `root` is the directory the file is
/// named from: a module's staged tree is flat, so its source root is put back, while a package
/// is staged where the reference compiles it and keeps the path as it lies.
fn source_coordinate(root: &str, rel: &std::path::Path) -> String {
    format!("{root}/{}.ets", source_key(rel))
}

/// `builder: Foo({ … })` is a component construction, and a `@CustomDialog` target is a dialog.
///
/// A user component is a class, so the call has to become a construction; left as a plain call
/// the runtime throws "class constructor cannot called without 'new'". A `@CustomDialog`
/// target is built inside the closure the runtime drives the dialog through: the closure
/// creates the instance, hands it the controller, registers it, and re-reads its params
/// (measured 2026-11-06: `fixtures/link-call-site` golden). Either form builds its params the
/// way a `build()` call site does, so a `@Link` member is handed the parent's observed
/// property rather than its value.
fn rewrite_dialog_builders(content: &str, source: &str) -> String {
    // `builder: SomeComponent({…})` builds a component, so it becomes a construction. The site
    // is read from the tree: an `ObjectProperty` whose key is `builder` and whose value calls a
    // registered component, so a `builder:` inside a string or a comment is not a candidate.
    let Some(sites) = crate::build::ets_transform::builder_sites(content) else {
        if crate::build::ets_transform::parse_diagnostics_enabled() {
            eprintln!("dialog builder rewrite skipped: the module does not parse");
        }
        return content.to_string();
    };
    let controllers = dialog_controllers(content);
    let page = crate::build::ets_compile::source_page().unwrap_or_default();
    // Where each written call sits in the source, and how many constructions of that shape the
    // emission has asked for so far. One source site backs every construction written from it:
    // the reference reads each construction's own node, so `ControlButton`'s two dialog fields —
    // emitted once in the constructor and once in `initialRender` — carry the same line twice.
    // The n-th construction of a shape therefore takes the n-th source occurrence and wraps
    // round when the source writes the shape fewer times.
    let mut source_calls: std::collections::HashMap<String, (Vec<(usize, usize)>, usize)> =
        std::collections::HashMap::new();
    let mut edits: Vec<(usize, usize, String)> = Vec::new();
    let mut owner_written: std::collections::BTreeSet<usize> = std::collections::BTreeSet::new();
    for site in sites {
        let Some(meta) = crate::build::compile::component_index::meta_of(&site.component) else {
            continue;
        };
        let mut params = crate::build::stage::process_custom_component::construct_params(
            &site.arguments,
            &meta.link_fields,
        );
        // `Foo()` passes no parameter object, and the reference still writes one: an empty `{}`.
        // `params` is spliced into the construction *and* into the `paramsLambda` body below, so
        // an empty string produced `new Foo(this, , undefined, …)` and `return ;` — a missing
        // slot is a syntax error, not an empty argument (measured:
        // `fixtures/customdialog-no-params`, whose golden line reads
        // `new ProbeDialog(this, {}, undefined, -1, …)`).
        if params.trim().is_empty() {
            params = "{}".to_string();
        }
        let controller = controllers
            .iter()
            .filter(|controller| controller.start < site.start && site.end < controller.close)
            .max_by_key(|controller| controller.start);
        let Some(controller) = controller else {
            // A builder argument outside a dialog controller still names a view the runtime
            // constructs; the shape there stays the inline construction.
            edits.push((
                site.start,
                site.end,
                format!(
                    "new {}(this, {params}, undefined, -1, () => {{}}, {{}})",
                    site.component
                ),
            ));
            continue;
        };
        // The controller carries its owner as the second argument. A controller already given
        // an owner is left alone.
        if !controller.has_owner {
            edits.push((controller.close, controller.close, ", this".to_string()));
            owner_written.insert(controller.close);
        }
        if !meta.is_dialog {
            edits.push((
                site.start,
                site.end,
                format!("{}({params})", site.component),
            ));
            continue;
        }
        // The dialog's own controller field is the one `setController` assigns.
        let Some(field) = &controller.field else {
            continue;
        };
        let entry = source_calls
            .entry(format!("{}({})", site.component, site.arguments))
            .or_insert_with(|| {
                (
                    source_call_positions(source, &site.component, &site.arguments),
                    0,
                )
            });
        let (line, column) = match entry.0.len() {
            0 => (0, 0),
            occurrences => {
                let position = entry.0[entry.1 % occurrences];
                entry.1 += 1;
                position
            }
        };
        edits.push((
            site.start,
            site.end,
            crate::build::stage::process_ui_syntax::custom_dialog_builder(
                &site.component,
                &params,
                crate::build::stage::process_ui_syntax::ControllerOwner {
                    name: field,
                    comments: &plain_member_comment_block(content, field),
                },
                &page,
                line,
                column,
                // The dialog arrow's statements land at column 16 in the reference
                // whatever indent the source wrote (five fixtures agree: sources at
                // 2 and 4 both print 16). `custom_dialog_builder` lays its statements at
                // `indent + 4` and closes the arrow at `indent`, so 12 is the value that
                // reproduces that depth — not the source line's indent.
                "            ",
            ),
        ));
    }
    // The owner argument is not a property of the builder: the reference appends it to every
    // `new CustomDialogController({ … })` written with its single object literal
    // (`createCustomDialogController`), whether or not the builder inside it is a component this
    // pass rewrites. `builder: this.SomeMethod` names a method of the component and takes the
    // owner all the same (`Xb_SettingItem`: `builder: this.ColorPickerDialog` has one).
    let owners =
        crate::build::ets_transform::dialog_controller_owner_sites(content).unwrap_or_default();
    for controller in &controllers {
        if controller.has_owner
            || owner_written.contains(&controller.close)
            || !owners.contains(&controller.start)
        {
            continue;
        }
        edits.push((controller.close, controller.close, ", this".to_string()));
    }
    apply_replacements(content, &edits)
}

/// One `new CustomDialogController(…)` construction in a module.
#[derive(Debug)]
struct DialogController {
    /// Offset of the `new` keyword.
    start: usize,
    /// Offset of the `)` closing its argument list.
    close: usize,
    /// The field the controller is stored in, when the construction is assigned to one.
    field: Option<String>,
    /// Whether the owner argument is already written.
    has_owner: bool,
}

/// Every `new CustomDialogController(…)` construction, in source order.
fn dialog_controllers(content: &str) -> Vec<DialogController> {
    let tokens: Vec<crate::build::lex::Token> = crate::build::lex::tokenize(content)
        .into_iter()
        .filter(|token| token.kind != crate::build::lex::Kind::Comment)
        .collect();
    let mut out = Vec::new();
    let mut index = 0usize;
    while index < tokens.len() {
        let is_new =
            tokens[index].kind == crate::build::lex::Kind::Ident && tokens[index].text == "new";
        let names_controller = tokens.get(index + 1).is_some_and(|token| {
            token.kind == crate::build::lex::Kind::Ident && token.text == "CustomDialogController"
        });
        if !is_new || !names_controller {
            index += 1;
            continue;
        }
        let Some(open) = tokens.get(index + 2).filter(|token| token.is_punct('(')) else {
            index += 1;
            continue;
        };
        let Some(close) = crate::build::lex::matching_delimiter(content, open.start) else {
            index += 1;
            continue;
        };
        let arguments = crate::build::lex::split_top_level_commas(&content[open.end..close]);
        out.push(DialogController {
            start: tokens[index].start,
            close,
            field: assigned_field(&tokens, index),
            has_owner: arguments.len() > 1,
        });
        index += 1;
    }
    out
}

/// The `this.<field>` an assignment stores the construction in, read from the tokens before it.
fn assigned_field(tokens: &[crate::build::lex::Token], at: usize) -> Option<String> {
    let assign = at.checked_sub(1)?;
    if tokens[assign].text != "=" {
        return None;
    }
    let field = assign.checked_sub(1)?;
    let dot = field.checked_sub(1)?;
    let owner = dot.checked_sub(1)?;
    let is_field = tokens[field].kind == crate::build::lex::Kind::Ident;
    let is_dot = tokens[dot].text == ".";
    let is_this =
        tokens[owner].kind == crate::build::lex::Kind::Ident && tokens[owner].text == "this";
    if !is_field || !is_dot || !is_this {
        return None;
    }
    Some(tokens[field].text.to_string())
}

/// The comment block above a plain member's declaration of `name`, read out of the text this
/// pass rewrites.
///
/// The reference prints a field's leading trivia where the controller's owner argument is
/// appended, but only for a member written without a decorator, without an accessibility
/// keyword and without `readonly` (measured: `fixtures/comment-leading`, where `private dlg`
/// carries nothing and `dlg2` carries `/* MEMBER_PLAIN */`).
fn plain_member_comment_block(content: &str, name: &str) -> Vec<String> {
    let lines: Vec<&str> = content.lines().collect();
    let Some(index) = lines
        .iter()
        .position(|line| is_member_declaration(line, name))
    else {
        return Vec::new();
    };
    let mut block: Vec<String> = Vec::new();
    let mut cursor = index;
    while cursor > 0 {
        let trimmed = lines[cursor - 1].trim();
        let is_comment = trimmed.starts_with("//")
            || trimmed.starts_with("/*")
            || trimmed.starts_with('*')
            || trimmed.ends_with("*/");
        if trimmed.is_empty() || !is_comment {
            break;
        }
        block.push(trimmed.to_string());
        cursor -= 1;
    }
    block.reverse();
    // A `*`-style block keeps one space before each continuation `*`: the reference prints
    // `/**` at the indent and every following line one column deeper
    // (`fixtures/…`, measured on `Network.ts`/`Overwrite.ts`/`Resources.ts`, whose block reads
    // `[0,17,17]`). The caller prefixes every line with the same indent, so the extra column is
    // carried by the line itself.
    for line in block.iter_mut().skip(1) {
        if line.starts_with('*') {
            line.insert(0, ' ');
        }
    }
    block
}

/// Whether `line` declares the member `name` — as opposed to the constructor assignment
/// `this.<name> = …`, which also carries the name.
fn is_member_declaration(line: &str, name: &str) -> bool {
    let mut rest = line.trim_start();
    for modifier in ["private", "public", "protected", "readonly", "static"] {
        if let Some(after) = rest.strip_prefix(modifier) {
            if after.starts_with(char::is_whitespace) {
                rest = after.trim_start();
            }
        }
    }
    let Some(after) = rest.strip_prefix(name) else {
        return false;
    };
    let after = after.strip_prefix('?').unwrap_or(after);
    after.starts_with(':')
}

/// The leading whitespace of the line `offset` sits on.
///
/// Only the dialog-arrow depth used to be read from it; the arrow's statements now sit at the
/// reference's fixed column, so the reading is kept for the test that pins its behaviour.
#[cfg(test)]
fn line_indent(text: &str, offset: usize) -> &str {
    let start = text[..offset.min(text.len())]
        .rfind('\n')
        .map_or(0, |at| at + 1);
    let rest = &text[start..];
    &rest[..rest.len() - rest.trim_start().len()]
}

/// Every place the source writes `component(args)`, in source order.
///
/// The text this pass reads is the lowered one, whose offsets do not hold the source's
/// coordinates, and the coordinate the reference writes is the source node's own (measured
/// 2026-11-06: `fixtures/link-call-site` golden carries line 8 for the call written on line 8).
/// The call is located by its token sequence, which the printer does not leave alone: it writes
/// the statement separators the source left to automatic semicolon insertion (`Xb_SettingItem`'s
/// `Xb_CustomDialog({…})` arguments carry four `;` that the source does not), and a separator is
/// not part of the call's shape. Both sides therefore skip it.
fn source_call_positions(source: &str, component: &str, args: &str) -> Vec<(usize, usize)> {
    let wanted: Vec<&str> = crate::build::lex::tokenize(args)
        .iter()
        .map(|token| token.text)
        .filter(|text| *text != ";")
        .collect();
    let tokens = crate::build::lex::tokenize(source);
    let mut positions = Vec::new();
    for (index, token) in tokens.iter().enumerate() {
        if token.kind != crate::build::lex::Kind::Ident || token.text != component {
            continue;
        }
        if !tokens.get(index + 1).is_some_and(|next| next.is_punct('(')) {
            continue;
        }
        let mut at = index + 2;
        let mut same_arguments = true;
        for want in &wanted {
            at = next_without_separators(&tokens, at);
            if tokens.get(at).map(|token| token.text) != Some(*want) {
                same_arguments = false;
                break;
            }
            at += 1;
        }
        if !same_arguments {
            continue;
        }
        let close = next_without_separators(&tokens, at);
        if !tokens.get(close).is_some_and(|token| token.is_punct(')')) {
            continue;
        }
        let position = crate::build::compile::ets_source_map::line_column(source, token.start);
        positions.push((position.line, position.column));
    }
    positions
}

/// The first token at or after `from` that is not a statement separator.
fn next_without_separators(tokens: &[crate::build::lex::Token<'_>], from: usize) -> usize {
    let mut at = from;
    while tokens.get(at).is_some_and(|token| token.text == ";") {
        at += 1;
    }
    at
}
/// The packages the staged sources import, by `[dependencies]` key.
fn used_packages(
    ts_root: &Path,
    packages: &[crate::build::deps::npm::Package],
) -> Result<std::collections::BTreeSet<String>, String> {
    let mut files: Vec<std::path::PathBuf> = Vec::new();
    collect_staged(ts_root, &mut files)?;
    let mut used = std::collections::BTreeSet::new();
    for file in &files {
        let Ok(text) = std::fs::read_to_string(file) else {
            continue;
        };
        for package in packages {
            let marker = format!("N&&&{}/", package.alias);
            let bare = format!("from '{}'", package.alias);
            let bare_double = format!("from \"{}\"", package.alias);
            if text.contains(&marker) || text.contains(&bare) || text.contains(&bare_double) {
                used.insert(package.name.clone());
            }
        }
    }
    Ok(used)
}

fn collect_staged(dir: &Path, out: &mut Vec<std::path::PathBuf>) -> Result<(), String> {
    let entries = std::fs::read_dir(dir).map_err(|e| format!("read {}: {e}", dir.display()))?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_staged(&path, out)?;
        } else {
            out.push(path);
        }
    }
    Ok(())
}

/// Whether a module's source declares `name` as a type (`interface` or `type`).
///
/// The declaration nodes decide it, `export`/`declare` modifiers included: those are flags on
/// the node, so a type specifier that only re-exports a name (`export { type X }`) is not read
/// as a declaration of it, which the old `type X` token pair could not tell apart.
///
/// A target the TypeScript parser rejects — a bare declaration list, or an `.ets` source with
/// ArkTS `struct`s — falls back to the token scan and says so on stderr.
fn declares_as_type(source: &str, name: &str) -> bool {
    match declared_type_names(source) {
        // A `const enum` is erased at its use sites, so the importing file has no runtime
        // binding to read: the reference writes that import as `import type`.
        Some(names) => {
            (names.iter().any(|declared| declared == name) && !declares_runtime_enum(source, name))
                || crate::build::lower::enum_init::declares_const_enum(source, name)
        }
        None => {
            if crate::build::ets_transform::parse_diagnostics_enabled() {
                eprintln!(
                    "type-only pass: the target module does not parse; reading `{name}` from its tokens"
                );
            }
            declares_as_type_by_tokens(source, name)
        }
    }
}

/// Every `interface` / `type` declaration name in a program, or `None` when it does not parse.
fn declared_type_names(source: &str) -> Option<Vec<String>> {
    use oxc_ast_visit::{walk, Visit};

    let allocator = Allocator::default();
    let ret = Parser::new(&allocator, source, SourceType::ts()).parse();
    if ret.panicked || !ret.errors.is_empty() {
        return None;
    }

    struct Collector {
        names: Vec<String>,
    }

    impl<'a> Visit<'a> for Collector {
        fn visit_ts_interface_declaration(&mut self, declaration: &TSInterfaceDeclaration<'a>) {
            self.names.push(declaration.id.name.to_string());
            walk::walk_ts_interface_declaration(self, declaration);
        }

        fn visit_ts_type_alias_declaration(&mut self, declaration: &TSTypeAliasDeclaration<'a>) {
            self.names.push(declaration.id.name.to_string());
            walk::walk_ts_type_alias_declaration(self, declaration);
        }
    }

    let mut collector = Collector { names: Vec::new() };
    collector.visit_program(&ret.program);
    Some(collector.names)
}

/// Whether `name` is an `enum` that carries runtime constants.
///
/// An enum is a value: its members are evaluated when the module loads, so a module that
/// imports it needs the binding at run time, not only in type positions. `declare` says the
/// module has no *declaration file side* — the SDK's `export declare enum KeyCode { … }` is
/// exactly that — but the emitted code still reads `KeyCode.KEYCODE_ESCAPE`, so a target that
/// declares it this way must not be classified type-only. The members are the evidence: an
/// initializer whose value is a number or a string is a constant the runtime carries.
///
/// The rule is read from the declaration's shape, never from the enum's or the module's name.
fn declares_runtime_enum(source: &str, name: &str) -> bool {
    use oxc_ast_visit::{walk, Visit};

    let allocator = Allocator::default();
    let ret = Parser::new(&allocator, source, SourceType::ts()).parse();
    if ret.panicked || !ret.errors.is_empty() {
        return false;
    }

    struct Collector<'a> {
        name: &'a str,
        found: bool,
    }

    impl<'a> Visit<'a> for Collector<'_> {
        fn visit_ts_enum_declaration(&mut self, declaration: &TSEnumDeclaration<'a>) {
            if declaration.id.name.as_str() == self.name {
                self.found |= declaration.body.members.iter().any(|member| {
                    member
                        .initializer
                        .as_ref()
                        .is_some_and(|value| value.is_literal())
                });
            }
            walk::walk_ts_enum_declaration(self, declaration);
        }
    }

    let mut collector = Collector { name, found: false };
    collector.visit_program(&ret.program);
    collector.found
}

/// The token-level reading of [`declares_as_type`], for a target the parser rejects.
fn declares_as_type_by_tokens(source: &str, name: &str) -> bool {
    let tokens = crate::build::lex::tokenize(source);
    tokens.windows(2).any(|pair| {
        pair[0].kind == crate::build::lex::Kind::Ident
            && (pair[0].text == "interface" || pair[0].text == "type")
            && pair[1].kind == crate::build::lex::Kind::Ident
            && pair[1].text == name
    })
}

/// Modules the runtime loads as native libraries rather than as JavaScript.
///
/// From the reference loader (`pre_define.js`): the system-api rewriter emits
/// `@native:<module>` for these and `@ohos:<module>` for everything else.  Getting this wrong
/// makes the runtime look for a JavaScript module that does not exist
/// (`export objects of native so is undefined`).
fn is_native_module(qualified: &str) -> bool {
    matches!(
        qualified,
        "system.app"
            | "ohos.app"
            | "system.router"
            | "system.curves"
            | "ohos.curves"
            | "system.matrix4"
            | "ohos.matrix4"
    )
}

/// Normalize a source module request the way the reference's system-api pass does:
/// `@ohos.curves` (native) → `@native:ohos.curves`, everything else → `@ohos:…`.
pub(crate) fn system_module_request(request: &str) -> String {
    // Kit configs name the declaration file (`@ohos.curves.d.ts`); the module is the stem.
    let request = request
        .strip_suffix(".d.ts")
        .or_else(|| request.strip_suffix(".d.ets"))
        .or_else(|| request.strip_suffix(".ts"))
        .unwrap_or(request);
    for prefix in ["@ohos.", "@hms.", "@system.", "@arkts."] {
        if let Some(rest) = request.strip_prefix(prefix) {
            let ns = prefix.trim_start_matches('@').trim_end_matches('.');
            if is_native_module(&format!("{ns}.{rest}")) {
                return format!("@native:{ns}.{rest}");
            }
            return format!("@{ns}:{rest}");
        }
    }
    request.to_string()
}

// One specifier plus the module context it resolves against; see `rewrite_relative_imports`.
#[allow(clippy::too_many_arguments)]
fn rewrite_spec(
    spec: &str,
    dir: &std::path::Path,
    module: &str,
    version: &str,
    packages: &[crate::build::deps::npm::Package],
    members: &[String],
    entry_module: &str,
    profile_record: &str,
) -> Option<String> {
    // A system module is addressed in its normalized form: the prefix is joined to the
    // module name with `:`, not `.` (`@ohos.file.fs` → `@ohos:file.fs`).  Without that the
    // runtime treats the specifier as an unresolvable package and the load fails.
    for prefix in ["@ohos.", "@hms.", "@system.", "@arkts."] {
        if let Some(rest) = spec.strip_prefix(prefix) {
            let ns = prefix.trim_start_matches('@').trim_end_matches('.');
            if is_native_module(&format!("{ns}.{rest}")) {
                return Some(format!("@native:{ns}.{rest}"));
            }
            return Some(format!("@{ns}:{rest}"));
        }
    }
    // A native library is addressed with the `Y` prefix (no package record behind it).
    if spec.ends_with(".so") {
        return Some(format!("@normalized:Y&&&{spec}&"));
    }
    // A bare specifier that names a sibling module: its package entry is `<module>/Index`,
    // and a specifier that already carries the source root keeps that path verbatim.
    if let Some(member) = members
        .iter()
        .find(|m| spec == m.as_str() || spec.starts_with(&format!("{m}/")))
    {
        let rest = spec[member.len()..].trim_start_matches('/');
        let member_version =
            crate::build::compile::abc_compile::record_version(member, entry_module);
        if rest.is_empty() {
            // A bare request names the member module's package entry, and the url path is that
            // entry, not the module source root: `getNormalizedOhmUrlByAliasName`
            // (`ark_utils.js:186`) composes `removeSuffix(alias + "/" + entryPath)`, which is
            // `proxy_core/Index` for `from 'proxy_core'`. The record carries the same name
            // (`abc_compile::module_entry_path`), and that agreement is what the device
            // resolves against. The entry module has no package entry, so a request that names
            // it keeps the source-root form.
            if *member != entry_module {
                return Some(format!(
                    "@normalized:N&&&{}&{member_version}",
                    crate::build::compile::abc_compile::module_entry_path(member)
                ));
            }
            return Some(format!(
                "@normalized:N&&&{}/{}/Index&{member_version}",
                member,
                crate::build::compile::abc_compile::MODULE_SOURCE_ROOT
            ));
        }
        return Some(format!("@normalized:N&&&{member}/{rest}&{member_version}"));
    }
    // `import BuildProfile from 'BuildProfile'` names a build-generated module: rewrite it to
    // the record the module actually carries.
    if spec == "BuildProfile" {
        return Some(format!(
            "@normalized:N&&&{}&{}",
            crate::build::compile::abc_compile::module_path(module, profile_record),
            version
        ));
    }
    if !spec.starts_with('.') {
        return package_url(spec, packages);
    }
    let mut parts: Vec<String> = dir
        .components()
        .map(|c| c.as_os_str().to_string_lossy().to_string())
        .collect();
    for seg in spec.split('/') {
        match seg {
            "." | "" => {}
            ".." => {
                parts.pop();
            }
            other => parts.push(other.to_string()),
        }
    }
    if let Some(last) = parts.last_mut() {
        for suffix in [".ets", ".ts", ".js"] {
            if let Some(stripped) = last.strip_suffix(suffix) {
                *last = stripped.to_string();
                break;
            }
        }
    }
    // A relative import that resolves onto the module's own package entry names that entry, so
    // it takes the same url as a bare request: `xb_components/entity/…` importing
    // `../../../../Index` emits `@normalized:N&&&xb_components/Index&1.0.0`, which is the name
    // the entry's record carries (`filesInfo.txt`: `&xb_components/Index&1.0.0`). Every other
    // relative import keeps the source-root form.
    let rel = parts.join("/");
    let path = if rel == crate::build::compile::abc_compile::module_entry(module)
        && module != entry_module
    {
        crate::build::compile::abc_compile::module_entry_path(module)
    } else {
        // The runtime resolves an import through the normalized url form and only then maps it
        // onto the record, so the specifier has to be the url, not the dotted name.
        crate::build::compile::abc_compile::module_path(module, &rel)
    };
    Some(format!("@normalized:N&&&{path}&{version}"))
}

/// Every name the project declares as a builder or a component struct. A call to one of
/// these takes a child block, and a struct may be spelled in lower case.
pub(crate) fn project_names(files: &[std::path::PathBuf]) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for f in files {
        let Ok(src) = std::fs::read_to_string(f) else {
            continue;
        };
        let (_masked, file_names, _e, _s) =
            crate::build::ets_compile::mask_builder_decorators(&src);
        for n in file_names {
            if !names.contains(&n) {
                names.push(n);
            }
        }
        for n in struct_names(&src) {
            if !names.contains(&n) {
                names.push(n);
            }
        }
    }
    names
}

/// Built-in ArkUI component names, read from the SDK's component definitions
/// (`ets-loader/components/*.json`, one `{"name": …}` per component).
/// The package specifiers the module sources import, relative ones excluded.
///
/// Only these packages are read for their declarations. An `oh_modules` tree of a real app
/// holds every installed package and the store's duplicates; the packages a build actually
/// imports are the ones whose names can appear in a field's type.
fn package_specifiers(source: &str) -> Vec<String> {
    let tokens = crate::build::lex::tokenize(source);
    let mut out: Vec<String> = Vec::new();
    for (index, token) in tokens.iter().enumerate() {
        if token.kind != crate::build::lex::Kind::Ident {
            continue;
        }
        if !matches!(token.text, "from" | "import") {
            continue;
        }
        let Some(next) = tokens.get(index + 1) else {
            continue;
        };
        if next.kind != crate::build::lex::Kind::Str {
            continue;
        }
        let specifier = next.text.trim_matches(|c| c == '\'' || c == '"');
        if specifier.is_empty() || specifier.starts_with('.') {
            continue;
        }
        let specifier = specifier.to_string();
        if !out.contains(&specifier) {
            out.push(specifier);
        }
    }
    out
}

/// Every `*.d.ets` / `*.d.ts` under a package directory, bounded by `depth`.
fn package_declarations(dir: &std::path::Path, depth: usize, out: &mut Vec<std::path::PathBuf>) {
    if depth > 8 {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in rd.flatten() {
        let path = entry.path();
        if path.is_dir() {
            package_declarations(&path, depth + 1, out);
            continue;
        }
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default();
        if name.ends_with(".d.ets") || name.ends_with(".d.ts") {
            out.push(path);
        }
    }
}

/// The declarations of the third-party packages the staged modules import.
///
/// The staging tree keeps a module's packages next to its sources (`<dir>/oh_modules`), so the
/// package root is a sibling of the directory the module was staged into, and no project-root
/// parameter has to travel through the pipeline.
fn package_declaration_sources(modules: &[StageModule<'_>], sources: &[String]) -> Vec<String> {
    let mut wanted: Vec<String> = Vec::new();
    for source in sources {
        for specifier in package_specifiers(source) {
            if !wanted.contains(&specifier) {
                wanted.push(specifier);
            }
        }
    }
    let mut paths: Vec<std::path::PathBuf> = Vec::new();
    for module in modules {
        // `oh_modules` lives either next to the module's sources (a multi-module project:
        // `xb_components/oh_modules`) or one level up (a single-module project, where the
        // module directory is `<project>/src`).
        let mut roots = vec![module.dir.join("oh_modules")];
        if let Some(parent) = module.dir.parent() {
            roots.push(parent.join("oh_modules"));
        }
        for root in roots {
            for specifier in &wanted {
                let dir = root.join(specifier);
                if dir.is_dir() {
                    package_declarations(&dir, 0, &mut paths);
                }
            }
        }
    }
    paths.sort();
    paths.dedup();
    paths
        .iter()
        .filter_map(|path| std::fs::read_to_string(path).ok())
        .collect()
}

/// The SDK's ambient declaration files, read from `<sdk>/ets/component/*.d.ts`.
///
/// The kit-config directory sits inside the SDK (`…/ets/build-tools/ets-loader/…`), so the SDK
/// root is one of its ancestors; the first ancestor that holds an `ets/component` directory is
/// the one. A project that pins no kit config reads no SDK files, and every SDK name then stays
/// unknown — the same answer the table gives before this hook exists.
fn sdk_declaration_sources(kit_config_dirs: &[std::path::PathBuf]) -> Vec<String> {
    let mut sources = Vec::new();
    for kit_config_dir in kit_config_dirs {
        let mut probe = kit_config_dir.as_path();
        for _ in 0..6 {
            let component_dir = probe.join("ets").join("component");
            if component_dir.is_dir() {
                let Ok(rd) = std::fs::read_dir(&component_dir) else {
                    break;
                };
                let mut paths: Vec<std::path::PathBuf> = rd
                    .flatten()
                    .map(|entry| entry.path())
                    // `Path::extension` answers `ts` for `enums.d.ts`, so the whole file name
                    // is what the check has to look at.
                    .filter(|path| {
                        path.file_name()
                            .map(|name| name.to_string_lossy().ends_with(".d.ts"))
                            .unwrap_or(false)
                    })
                    .collect();
                paths.sort();
                for path in paths {
                    if let Ok(text) = std::fs::read_to_string(&path) {
                        sources.push(text);
                    }
                }
                break;
            }
            match probe.parent() {
                Some(parent) => probe = parent,
                None => break,
            }
        }
    }
    sources
}

pub(crate) fn sdk_component_names(kit_config_dirs: &[std::path::PathBuf]) -> Vec<String> {
    let mut names = Vec::new();
    for kit_config_dir in kit_config_dirs {
        let dir = kit_config_dir
            .parent()
            .map(|p| p.join("components"))
            .unwrap_or_default();
        let Ok(rd) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in rd.flatten() {
            let path = entry.path();
            if path.extension().map(|x| x != "json").unwrap_or(true) {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
                continue;
            };
            if let Some(name) = value.get("name").and_then(|n| n.as_str()) {
                if !names.contains(&name.to_string()) {
                    names.push(name.to_string());
                }
            }
        }
    }
    names
}

/// `struct X` declarations in one file.
pub(crate) fn struct_names(src: &str) -> Vec<String> {
    // `struct` is a keyword, so it arrives as its own token; the name is the next one.
    let tokens = crate::build::lex::tokenize(src);
    let mut out = Vec::new();
    for pair in tokens.windows(2) {
        if pair[0].kind == crate::build::lex::Kind::Ident
            && pair[0].text == "struct"
            && pair[1].kind == crate::build::lex::Kind::Ident
        {
            out.push(pair[1].text.to_string());
        }
    }
    out
}

fn collect_ets(dir: &Path, out: &mut Vec<std::path::PathBuf>) -> Result<(), String> {
    let entries = std::fs::read_dir(dir).map_err(|e| format!("read_dir {}: {e}", dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("entry: {e}"))?;
        let path = entry.path();
        if path.is_dir() {
            collect_ets(&path, out)?;
        } else if path.extension().is_some_and(|e| e == "ets" || e == "ts") {
            // Declaration files describe types and ship no runtime code.
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name.ends_with(".d.ets") || name.ends_with(".d.ts") {
                continue;
            }
            // Plain `.ts` sources are compiled exactly like `.ets`: an imported module is a
            // record whichever extension it carries, and dropping the `.ts` ones leaves the
            // import graph half-populated.
            out.push(path);
        }
    }
    Ok(())
}

/// Every source reachable from `entries` through relative imports, `entries` included.
///
/// A root is module-relative (`pages/Index.ets`); an import may omit the extension or name a
/// directory, so candidates are tried in the resolver's order. A bare specifier names a system
/// module, a package or a sibling module — none of them a file of this tree — and is skipped.
fn source_closure(
    root: &Path,
    entries: &[String],
) -> Result<(Vec<std::path::PathBuf>, StagingPaths), String> {
    let mut queued: std::collections::BTreeSet<std::path::PathBuf> =
        std::collections::BTreeSet::new();
    let mut pending: Vec<std::path::PathBuf> = Vec::new();
    let mut staged_at: StagingPaths = StagingPaths::new();
    for entry in entries {
        // An ability declaration carries a `dir` that may not match where the file actually
        // sits (a project can move the source without editing the descriptor). The reference
        // resolves entries through its own dependency graph and never trips on this, so a
        // declared path that does not exist falls back to the file name inside the module.
        let path = match resolve_entry(root, entry) {
            Some(path) => path,
            None => match resolve_by_name(root, entry)? {
                Some(path) => {
                    // The declared path still names the record: the device loads the ability
                    // through module.json's `srcEntry` (`…/entrybackupability/EntryBackupAbility`),
                    // while the flat tree keeps the file at another path. Staging it where the
                    // file happens to sit produced a record no `srcEntry` points to.
                    let declared = entry.trim_start_matches("./");
                    let declared = declared.strip_prefix("ets/").unwrap_or(declared);
                    staged_at.insert(path.clone(), std::path::PathBuf::from(declared));
                    path
                }
                // Nothing by that name exists anywhere in the module: the file is genuinely
                // missing, and silently dropping it would hide a broken build.
                None => return Err(format!("{}: entry {entry} not found", root.display())),
            },
        };
        if queued.insert(path.clone()) {
            pending.push(path);
        }
    }
    let mut files = Vec::new();
    while let Some(path) = pending.pop() {
        let text =
            std::fs::read_to_string(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
        let from = path
            .parent()
            .and_then(|dir| dir.strip_prefix(root).ok())
            .unwrap_or_else(|| Path::new(""));
        for request in crate::build::ets_transform::module_requests(&text) {
            let Some(next) = resolve_source(root, from, &request) else {
                continue;
            };
            if queued.insert(next.clone()) {
                pending.push(next);
            }
        }
        files.push(path);
    }
    Ok((files, staged_at))
}

/// Staging paths that differ from a source's own path, keyed by source.
type StagingPaths = std::collections::BTreeMap<std::path::PathBuf, std::path::PathBuf>;

/// The file an entry names when the declared directory is wrong: the same base name somewhere
/// inside the module. Only an unambiguous hit counts.
fn resolve_by_name(root: &Path, entry: &str) -> Result<Option<std::path::PathBuf>, String> {
    let name = std::path::Path::new(entry)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(entry);
    let mut found: Vec<std::path::PathBuf> = Vec::new();
    collect_named(root, name, &mut found)?;
    Ok((found.len() == 1).then(|| found.remove(0)))
}

/// Depth-first search for a source file with this exact file name.
fn collect_named(
    dir: &Path,
    name: &str,
    found: &mut Vec<std::path::PathBuf>,
) -> Result<(), String> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return Ok(()),
    };
    for item in entries.flatten() {
        let path = item.path();
        if path.is_dir() {
            collect_named(&path, name, found)?;
        } else if path.file_name().and_then(|value| value.to_str()) == Some(name)
            && is_source_file(&path)
        {
            found.push(path);
        }
    }
    Ok(())
}

/// The file a compile root names. `module.json`'s `srcEntry` is spelled against the `ets/` root
/// the module directory stands for, so that prefix is dropped here.
fn resolve_entry(root: &Path, entry: &str) -> Option<std::path::PathBuf> {
    let entry = entry.trim_start_matches("./");
    let entry = entry.strip_prefix("ets/").unwrap_or(entry);
    let base = root.join(entry);
    if is_source_file(&base) {
        return Some(base);
    }
    ["ets", "ts"]
        .iter()
        .flat_map(|ext| {
            [
                std::path::PathBuf::from(format!("{}.{ext}", base.display())),
                base.with_extension(ext),
            ]
        })
        .find(|path| is_source_file(path))
}

/// The file a relative specifier names, resolved the way the reference's TypeScript resolver
/// does: the extension may be omitted and a directory names its `index` file.
fn resolve_source(root: &Path, from: &Path, spec: &str) -> Option<std::path::PathBuf> {
    if !spec.starts_with('.') {
        return None;
    }
    let mut parts: Vec<String> = from
        .components()
        .filter_map(|component| match component {
            std::path::Component::Normal(name) => Some(name.to_string_lossy().to_string()),
            _ => None,
        })
        .collect();
    for seg in spec.split('/') {
        match seg {
            "." | "" => {}
            ".." => {
                parts.pop();
            }
            other => parts.push(other.to_string()),
        }
    }
    let base = root.join(parts.join("/"));
    let mut candidates = Vec::new();
    if base
        .extension()
        .is_some_and(|ext| ext == "ets" || ext == "ts")
    {
        candidates.push(base.clone());
    }
    for ext in ["ets", "ts"] {
        // The extension is appended, not substituted: `./charts.class` names
        // `charts.class.ts`, and `with_extension` would look for `charts.ts` instead
        // (`@mcui/mccharts` ships that shape, and the file then never reached the package
        // closure, so its record was missing at run time).
        candidates.push(std::path::PathBuf::from(format!(
            "{}.{ext}",
            base.display()
        )));
        candidates.push(base.with_extension(ext));
        candidates.push(base.join(format!("index.{ext}")));
    }
    candidates.into_iter().find(|path| is_source_file(path))
}

/// A `.ets`/`.ts` file that ships code: a declaration file describes types and is never a
/// record of the merged abc.
fn is_source_file(path: &Path) -> bool {
    if !path.is_file()
        || !path
            .extension()
            .is_some_and(|ext| ext == "ets" || ext == "ts")
    {
        return false;
    }
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    !name.ends_with(".d.ets") && !name.ends_with(".d.ts")
}

/// The url a bare import specifier resolves to when it names a declared package.
///
/// `yaml` addresses the package entry, `@cxy/sandboxfinder/src/main/ets/utils/X` addresses a
/// module inside it.  A specifier naming no declared package resolves to nothing and is left
/// for the compiler (system modules such as `@ohos:file.fs` take that path).
fn package_url(spec: &str, packages: &[crate::build::deps::npm::Package]) -> Option<String> {
    for package in packages {
        if spec == package.name {
            return Some(package_entry_url(package));
        }
        if let Some(rest) = spec.strip_prefix(&format!("{}/", package.name)) {
            let rest = rest
                .trim_end_matches(".js")
                .trim_end_matches(".ts")
                .trim_end_matches(".ets");
            return Some(package.url(rest));
        }
    }
    None
}

/// The url a bare import of a third-party package resolves to.
///
/// A bare request names the package's **entry**, and the entry is the file the package's own
/// manifest declares. The reference records it as `pkgContextInfo.<pkg>.entryPath` — `yaml`
/// resolves to `browser/index.js`, `@mcui/mccharts` to `index.ets` — and
/// `getNormalizedOhmUrlByAliasName` (`ark_utils.js:186`) composes the request as
/// `removeSuffix(alias + "/" + entryPath)`: the extension is dropped and the path is taken as
/// the manifest writes it, relative to the package directory.
///
/// The declared target undergoes the manifest's `browser` substitution: a package that ships
/// a browser bundle is compiled from that bundle alone (`Package::source_root`), and the
/// substituted entry sits at the bundle's root under the declared file's name
/// (`./dist/index.js` → `./browser/index.js`, so `browser/index`). When the declared file
/// exists at its own path and the bundle carries no file of that name, the declared path is
/// the entry as written.
fn package_entry_url(package: &crate::build::deps::npm::Package) -> String {
    let path = package_entry_path(package).unwrap_or_else(|| package.entry.clone());
    format!(
        "@normalized:N&&&{}/{}&{}",
        package.alias, path, package.version
    )
}

/// The package-directory-relative entry path of one package, without its extension.
fn package_entry_path(package: &crate::build::deps::npm::Package) -> Option<String> {
    let declared = declared_package_main(&package.dir)?;
    let file = declared.rsplit('/').next().unwrap_or(&declared).to_string();
    let stem = |name: &str| {
        let mut name = name.to_string();
        for suffix in [".d.ets", ".d.ts", ".ets", ".ts", ".js", ".mjs", ".cjs"] {
            if let Some(stripped) = name.strip_suffix(suffix) {
                name = stripped.to_string();
                break;
            }
        }
        name
    };
    // The bundle's own copy of the declared entry wins: `browser/index.js` for `dist/index.js`.
    let bundle = package
        .source_root
        .strip_prefix(&package.dir)
        .ok()
        .map(|rest| rest.to_string_lossy().replace('\\', "/"))
        .filter(|rest| !rest.is_empty());
    if let Some(bundle) = &bundle {
        if package.source_root.join(&file).is_file() {
            return Some(format!("{bundle}/{}", stem(&file)));
        }
    }
    Some(stem(declared.trim_start_matches("./")))
}

/// The `main` a package's own manifest declares, as written (`./dist/index.js`).
fn declared_package_main(dir: &std::path::Path) -> Option<String> {
    let text = std::fs::read_to_string(dir.join("oh-package.json5"))
        .or_else(|_| std::fs::read_to_string(dir.join("package.json")))
        .ok()?;
    let start = text.find("\"main\"")? + "\"main\"".len();
    let rest = &text[start..];
    let open = rest.find('"')?;
    let rest = &rest[open + 1..];
    let close = rest.find('"')?;
    let value = rest[..close].trim().to_string();
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

/// Copy one package's sources into the staging tree, rewriting each file's imports the way
/// the device resolves them.
/// The package-internal files the module imports, spelled as paths relative to the package's
/// source root without an extension (`settings/SettingsCommonOp`). A bare package import names
/// the entry and is recorded as the empty string.
fn imported_package_paths(
    ts_root: &Path,
    packages: &[crate::build::deps::npm::Package],
    package: &crate::build::deps::npm::Package,
) -> Result<std::collections::BTreeSet<String>, String> {
    let mut files: Vec<std::path::PathBuf> = Vec::new();
    collect_staged(ts_root, &mut files)?;
    let mut out = std::collections::BTreeSet::new();
    for file in &files {
        // Package sources are staged under `<root>/<package alias>/…`; their own imports are
        // collected by the closure walk, not here.
        if file.starts_with(ts_root.join(package.staging_dir())) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(file) else {
            continue;
        };
        for request in crate::build::ets_transform::module_requests(&text)
            .into_iter()
            .chain(
                crate::build::deps::npm::dynamic_requests(&text)
                    .into_iter()
                    .map(|request| request.request),
            )
        {
            // A staged module already carries the normalized form
            // (`@normalized:N&&&<alias>/<path>&<version>`), so the alias is read out of that
            // shape; a source that still writes the bare name is handled the same way.
            let normalized = request.strip_prefix("@normalized:").unwrap_or(&request);
            let normalized = normalized.strip_prefix("N&&&").unwrap_or(normalized);
            let path_part = normalized.split('&').next().unwrap_or(normalized);
            let Some(rest) = path_part.strip_prefix(&package.alias) else {
                continue;
            };
            if !rest.is_empty() && !rest.starts_with('/') {
                continue;
            }
            out.insert(rest.trim_start_matches('/').to_string());
        }
    }
    let _ = packages;
    Ok(out)
}

/// Every package file reachable from the given entry paths, following relative imports inside
/// the package.
fn package_closure(
    source_root: &Path,
    entries: &std::collections::BTreeSet<String>,
) -> Result<std::collections::BTreeSet<std::path::PathBuf>, String> {
    let mut seen: std::collections::BTreeSet<std::path::PathBuf> =
        std::collections::BTreeSet::new();
    let mut pending: Vec<std::path::PathBuf> = Vec::new();
    for entry in entries {
        let base = if entry.is_empty() {
            source_root.to_path_buf()
        } else {
            source_root.join(entry)
        };
        if let Some(path) = resolve_source(
            source_root,
            Path::new(""),
            &format!(
                "./{}",
                base.strip_prefix(source_root).unwrap_or(&base).display()
            ),
        ) {
            pending.push(path);
        } else if let Some(path) = resolve_entry(source_root, &base.display().to_string()) {
            pending.push(path);
        }
    }
    while let Some(path) = pending.pop() {
        if !seen.insert(path.clone()) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let from = path
            .parent()
            .and_then(|dir| dir.strip_prefix(source_root).ok())
            .unwrap_or_else(|| Path::new(""));
        for request in crate::build::ets_transform::module_requests(&text) {
            if let Some(next) = resolve_source(source_root, from, &request) {
                pending.push(next);
            }
        }
    }
    Ok(seen)
}

fn stage_package(
    package: &crate::build::deps::npm::Package,
    ts_root: &Path,
    kit_configs_dir: &[std::path::PathBuf],
    project_builders: &[String],
    kits: &std::collections::BTreeMap<
        String,
        std::collections::BTreeMap<String, crate::build::stage::process_import::KitEntry>,
    >,
    wanted: &std::collections::BTreeSet<String>,
) -> Result<(), String> {
    let mut files: Vec<std::path::PathBuf> = Vec::new();
    collect_package_sources(&package.source_root, &mut files)?;
    // The reference compiles only the part of a package the module reaches. Staging the whole
    // tree pulls in files nothing imports, and their records make the merge disagree with the
    // reference (measured on clashbox: 78 package files staged against 59 compiled).
    //
    // Known difference, not a pending bug: this walk follows the **source** import graph, while
    // the reference follows the graph its transform leaves behind. Where the two disagree the
    // extra file stays staged: `@mcui/mccharts`'s `utils/defaultOption` is imported by
    // `pageView/Charts.ets` in the source, but the transform erases that statement as unread
    // (the five names only ever occur there as the struct's own field names), so the reference
    // never compiles the file and we stage and record it unreferenced. It resolves — the record
    // exists — so the package still loads; the cost is one unused entry in the staged tree.
    // Switching this to the post-transform graph means transforming before the file set is
    // known; the erasure rules live in the lowering and still move, which is how the 60-vs-59
    // staged/compiled mismatch came back the first time. A post-staging reachability pass would
    // be the shape that does not depend on them, and nothing needs it today.
    if !wanted.is_empty() {
        let reachable = package_closure(&package.source_root, wanted)?;
        // A closure that resolves nothing means the specifiers are shaped differently from what
        // this walk understands; keeping the whole tree is the safe side, and the staged count
        // then shows up in the merge as before.
        if !reachable.is_empty() {
            files.retain(|file| reachable.contains(file));
        }
    }
    // Several files can map to the same staged name; the package's declared entry is the
    // implementation, so it is written last and wins.
    if let Some(entry_file) = crate::build::deps::npm::entry_file_name(package) {
        files.sort_by_key(|f| {
            let name = f.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name == entry_file {
                1
            } else {
                0
            }
        });
    }
    if files.is_empty() {
        return Err(format!(
            "{}: package has no sources under {}",
            package.name,
            package.source_root.display()
        ));
    }
    let target_root = ts_root.join(package.staging_dir());
    for file in files {
        let Ok(rel) = file.strip_prefix(&package.source_root) else {
            continue;
        };
        // The compiler only recognises `.ts`/`.js` inputs; an `.ets` source is TypeScript
        // once its component syntax is expanded, so it is staged under that extension.
        let dest = target_root.join(rel).with_extension("ts");
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("mkdir {}: {e}", parent.display()))?;
        }
        // A package source is not a page of this project and has no route to name it, so the
        // staging loop is the only place its coordinate can be set: the reference reads the
        // file's own path there too (`path.relative(cwd, resourceFileName)`), which is the
        // staged path under `oh_modules/.ohpm` with its `.ets` extension.
        crate::build::ets_compile::set_source_page(&source_coordinate(&package.staging_dir(), rel));
        // A package's sources resolve against the package, the same owner the index recorded
        // their declarations under: a component call inside the package names a sibling, and
        // an import names the package (or another one).
        let previous_source = crate::build::compile::component_index::current_source();
        let key = source_key(rel);
        crate::build::compile::component_index::set_current_source(&package.alias, &key);
        let src =
            std::fs::read_to_string(&file).map_err(|e| format!("read {}: {e}", file.display()))?;
        // A component library is ArkTS, not plain TypeScript: its `struct`/`@Component`
        // spelling has to go through the same transform the project's own pages do.
        let content = if is_arkts(&src) {
            let opts = TranspileOptions {
                kit_config_dirs: kit_configs_dir,
                project_builders,
                route: None,
            };
            // A component library may park a whole earlier version of a file in leading
            // comments; the transform would otherwise read that copy as the live component.
            let live = drop_live_comment_lines(&src);
            crate::build::lower::ets_transpile::transpile(&live, &opts)
                .map_err(|e| format!("{}: {e}", file.display()))?
        } else {
            // No component: a plain module that may still declare `@Builder` functions,
            // which carry component child blocks and need the same masking.
            crate::build::lower::ets_plain::lower(&src, kits)
                .map_err(|e| format!("{}: {e}", file.display()))?
        };
        // A package source is erased the same way a module source is: an import that only
        // carries types becomes `import type` (the compiler then drops it), and a mixed
        // import is split so the runtime names stay values. `rel` is the package-relative
        // location, so a relative request resolves inside the package's own tree.
        let content = split_package_type_imports(&content, package, rel);
        // The package's own files get the same emission the module sources get: unread
        // bindings erased, a mixed statement split, the type half written `import type`. The
        // pass's request closure is the identity here because the package's urls are resolved
        // by `rewrite_package_imports` below, which decides per request from the emitted text.
        // The type decision needs the target's declarations as well: a name the target
        // declares as an `interface`/`type` has no runtime export, so treating it as a value
        // makes the device fail module instantiation
        // (`… does not provide an export name 'IClashManager'`, measured on clashbox).
        let package_dir = rel.parent().unwrap_or_else(|| std::path::Path::new(""));
        let content = crate::build::ets_transform::rewrite_imports(
            &content,
            Some(&src),
            &|_| None,
            &|request, name| {
                resolve_source(&package.source_root, package_dir, request)
                    .and_then(|target| std::fs::read_to_string(target).ok())
                    .is_some_and(|text| declares_as_type(&text, name))
            },
        )
        .unwrap_or(content);
        let content = rewrite_package_imports(&content, package, rel);
        std::fs::write(&dest, content).map_err(|e| format!("write {}: {e}", dest.display()))?;
        crate::build::compile::component_index::restore_source(previous_source);
    }
    Ok(())
}

/// Every `.js`/`.ts`/`.ets` file of a package, test and typings excluded.
fn collect_package_sources(dir: &Path, out: &mut Vec<std::path::PathBuf>) -> Result<(), String> {
    let entries = std::fs::read_dir(dir).map_err(|e| format!("read {}: {e}", dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("read {}: {e}", dir.display()))?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if path.is_dir() {
            if matches!(name.as_str(), "node_modules" | "test" | "tests" | ".git") {
                continue;
            }
            collect_package_sources(&path, out)?;
            continue;
        }
        let is_source = path
            .extension()
            .is_some_and(|e| matches!(e.to_str(), Some("js" | "ts" | "ets")));
        if is_source && !name.ends_with(".d.ts") {
            out.push(path);
        }
    }
    Ok(())
}

/// The `api` directory of each SDK level, next to the kit configs the build resolves.
///
/// The layout is `<ets>/build-tools/ets-loader/kit_configs` and the declarations sit in
/// `<ets>/api`, so the directory is three levels up from a kit config.
fn sdk_api_dirs(kit_config_dirs: &[std::path::PathBuf]) -> Vec<std::path::PathBuf> {
    let mut dirs: Vec<std::path::PathBuf> = Vec::new();
    for config in kit_config_dirs {
        let Some(api) = config.ancestors().nth(3).map(|ets| ets.join("api")) else {
            continue;
        };
        if api.is_dir() && !dirs.contains(&api) {
            dirs.push(api);
        }
    }
    dirs
}

/// The declaration file a system module request names, when the SDK ships one.
///
/// A kit's table states the module the way the output spells it (`@ohos:arkui.advanced.SegmentButton`)
/// while a source may also import the module itself (`@ohos.arkui.advanced.SegmentButton`); both
/// name `@ohos.arkui.advanced.SegmentButton.d.ets`. Only the ArkTS declaration extension is
/// tried: a component is a `struct`, which is ArkTS syntax, so a `.d.ts` never declares one.
fn sdk_declaration_file(
    api_dirs: &[std::path::PathBuf],
    request: &str,
) -> Option<std::path::PathBuf> {
    let module = match request.split_once(':') {
        Some((namespace, rest)) if namespace.starts_with('@') => format!("{namespace}.{rest}"),
        _ => request.to_string(),
    };
    api_dirs
        .iter()
        .map(|dir| dir.join(format!("{module}.d.ets")))
        .find(|path| path.is_file())
}

/// The declaration files one import reaches, when the import names a module with no compiled
/// source.
fn declaration_files_for_import(
    request: &str,
    local: &str,
    api_dirs: &[std::path::PathBuf],
    packages: &[crate::build::deps::npm::Package],
) -> Vec<std::path::PathBuf> {
    // `import { SegmentButton } from "@kit.ArkUI"`: the kit's own table maps the exported name
    // to the module that declares it.
    if request.starts_with("@kit.") {
        let Some(table) = crate::build::stage::process_import::kit_table(request) else {
            return Vec::new();
        };
        let Some((module, _)) = table.get(local) else {
            return Vec::new();
        };
        return sdk_declaration_file(api_dirs, module).into_iter().collect();
    }
    // A system module imported by its own name names the declaration file itself.
    if request.starts_with('@') && !request.starts_with("@normalized:") {
        if let Some(path) = sdk_declaration_file(api_dirs, request) {
            return vec![path];
        }
    }
    // A package: its entry is the file the import reaches, and the components it declares are
    // the ones its entry re-exports. A package whose sources the index already scanned
    // (`Kind::Arkts`) needs nothing here.
    let Some(package) = packages
        .iter()
        .find(|package| package.alias == request || package.name == request)
    else {
        return Vec::new();
    };
    if package.kind == crate::build::deps::npm::Kind::Arkts {
        return Vec::new();
    }
    package_declaration_files(package)
}

/// The declaration files a package's entry re-exports.
///
/// A package that ships only declarations (`Kind::ByteCode`) keeps its components behind its
/// entry (`Index.d.ets`), which re-exports one file per component; the walk follows exactly
/// those `export … from "./…"` statements, so a declaration the package does not export is never
/// read. The bound is a guard on package data, not on project data.
fn package_declaration_files(
    package: &crate::build::deps::npm::Package,
) -> Vec<std::path::PathBuf> {
    let mut out: Vec<std::path::PathBuf> = Vec::new();
    let mut seen: std::collections::BTreeSet<std::path::PathBuf> =
        std::collections::BTreeSet::new();
    let mut pending: Vec<std::path::PathBuf> = Vec::new();
    for candidate in [
        package.source_root.join("Index.d.ets"),
        package.dir.join(format!("{}.d.ets", package.entry)),
    ] {
        if candidate.is_file() && seen.insert(candidate.clone()) {
            pending.push(candidate);
        }
    }
    while let Some(path) = pending.pop() {
        if seen.len() > 256 {
            break;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        out.push(path.clone());
        let Some(statements) = crate::build::ets_transform::import_statements(&text) else {
            continue;
        };
        for statement in statements.iter().filter(|statement| statement.is_export) {
            if !statement.request.starts_with('.') {
                continue;
            }
            let Some(next) = relative_declaration_file(&path, &statement.request) else {
                continue;
            };
            if seen.insert(next.clone()) {
                pending.push(next);
            }
        }
    }
    out.retain(|path| path.to_string_lossy().ends_with(".d.ets"));
    out
}

/// The file a relative request in a declaration file names.
///
/// A re-export drops the extension (`from "./Dialog"`) and the declaration of a module is
/// `Dialog.d.ets`; the compiled source is a candidate too, so a package that re-exports one is
/// left out of the declaration scan rather than mistaken for one.
fn relative_declaration_file(from: &std::path::Path, request: &str) -> Option<std::path::PathBuf> {
    let base = from.parent()?.join(request);
    let mut candidates: Vec<std::path::PathBuf> = Vec::new();
    if base.extension().is_some() {
        candidates.push(base.clone());
    }
    for suffix in [".d.ets", ".ets", ".ts", ".d.ts"] {
        let mut name = base.clone().into_os_string();
        name.push(suffix);
        candidates.push(std::path::PathBuf::from(name));
    }
    candidates.push(base.join("Index.d.ets"));
    candidates.into_iter().find(|path| path.is_file())
}

/// Rewrite a package-internal file's imports to the package's own urls.
fn rewrite_package_imports(
    content: &str,
    package: &crate::build::deps::npm::Package,
    rel: &std::path::Path,
) -> String {
    let dir = rel.parent().unwrap_or_else(|| std::path::Path::new(""));
    // The specifiers come from the tree, so a `from` inside a comment or a string cannot
    // split a statement, and a binding list spread over several lines is one node.
    let Some(specifiers) = crate::build::ets_transform::module_specifiers(content) else {
        if crate::build::ets_transform::parse_diagnostics_enabled() {
            eprintln!(
                "package import rewrite skipped: {} does not parse",
                rel.display()
            );
        }
        return content.to_string();
    };
    // The urls are resolved per request: a request some statement in this file loads at
    // runtime is addressed by the package url on every statement that names it, the
    // type-only copies included (`proxy_core/Index.ets` normalizes `export type
    // { UpdateConfigParams }` because a value statement names the same `models/ClashConfig`).
    // The set is read from the emitted text, after `rewrite_imports` has split the
    // statements: what it keeps as a value statement is what the file runs, and what it
    // writes as `import type` the compiler erases.
    let loaded = crate::build::ets_transform::loaded_requests(&specifiers);
    let mut edits: Vec<(usize, usize, String)> = Vec::new();
    for specifier in &specifiers {
        let quote = &content[specifier.start..specifier.start + 1];
        let spec = specifier.request.as_str();
        // A system module keeps its normalized form inside packages too: a dotted specifier is
        // treated as a package name and fails to resolve from a package record. The
        // system-api rewrite is not a usage decision, so a type-only statement takes it too.
        let mut system = None;
        for prefix in ["@ohos.", "@hms.", "@system.", "@arkts."] {
            if let Some(tail) = spec.strip_prefix(prefix) {
                let ns = prefix.trim_start_matches('@').trim_end_matches('.');
                system = Some(if is_native_module(&format!("{ns}.{tail}")) {
                    format!("@native:{ns}.{tail}")
                } else {
                    format!("@{ns}:{tail}")
                });
                break;
            }
        }
        if let Some(normalized) = system {
            edits.push((
                specifier.start,
                specifier.end,
                format!("{quote}{normalized}{quote}"),
            ));
            continue;
        }
        // A statement that only carries types is erased by the compiler, so the reference
        // leaves its request as written when nothing in the file loads it: `@mcui/mccharts`'s
        // `import type { InterfaceObj } from '../utils/chartInterface'` (measured on clashbox,
        // 2026-11-06) and `ApiHandler.ts`'s `import type { FileManager } from
        // '../manager/FileManager'` — the target is a class, so the declaration pass cannot
        // call it a type; only the importing file's usage can, and the request stays relative
        // either way. A statement that still carries a value binding is normalized onto the
        // record url.
        if specifier.type_only && !loaded.contains(spec) {
            continue;
        }
        let target = if let Some(stripped) = spec.strip_prefix("./") {
            join_relative(dir, stripped)
        } else if let Some(stripped) = spec.strip_prefix("../") {
            join_relative(dir, &format!("../{stripped}"))
        } else if let Some(target) = bare_in_package(spec, package) {
            target
        } else {
            continue;
        };
        // A directory specifier resolves to its `index` file; the record name carries that
        // segment, so leaving it off names a record that does not exist.
        let target = if package_entry_missing(package, &target) {
            format!("{target}/index")
        } else {
            target
        };
        edits.push((
            specifier.start,
            specifier.end,
            format!("{quote}{}{quote}", package.url(&target)),
        ));
    }
    apply_replacements(content, &edits)
}

/// Split a package's import statements into their value and type parts.
///
/// The reference's type pass does the same (measured on clashbox's `@mcui/mccharts`,
/// 2026-11-06): `import { Chart, Options } from './Charts'` becomes
/// `import { Chart } from "@normalized:…"` plus `import type { Options } from
/// "@normalized:…"`, while a statement whose every binding is a type becomes one
/// `import type` that keeps the path as written. A binding is a type when the target module
/// declares it as `interface`/`type`; an unused value binding is not guessed at, because a
/// re-export (`import { x } …; export { x }`) uses the name outside the import statement.
fn split_package_type_imports(
    content: &str,
    package: &crate::build::deps::npm::Package,
    rel: &std::path::Path,
) -> String {
    let Some(statements) = crate::build::ets_transform::import_statements(content) else {
        return content.to_string();
    };
    let dir = rel.parent().unwrap_or_else(|| std::path::Path::new(""));
    let mut edits: Vec<(usize, usize, String)> = Vec::new();
    for statement in statements {
        if statement.is_export || statement.type_only || statement.default_binding.is_some() {
            continue;
        }
        if statement.bindings.is_empty() {
            continue;
        }
        let Some(target) = resolve_source(&package.source_root, dir, &statement.request) else {
            continue;
        };
        let Ok(text) = std::fs::read_to_string(target) else {
            continue;
        };
        let (types, values): (Vec<_>, Vec<_>) = statement
            .bindings
            .iter()
            .partition(|(symbol, _)| declares_as_type(&text, symbol));
        if types.is_empty() {
            continue;
        }
        let render = |(symbol, bind): &(String, String)| {
            if symbol == bind {
                symbol.clone()
            } else {
                format!("{symbol} as {bind}")
            }
        };
        let original = &content[statement.start..statement.end];
        let indent = &content[..statement.start]
            .rsplit_once('\n')
            .map(|(_, tail)| tail)
            .unwrap_or("");
        let indent = indent.trim_end_matches(|c: char| !c.is_whitespace());
        let quote = if original.contains("from \"") {
            '"'
        } else {
            '\''
        };
        let request = &statement.request;
        let named = |items: Vec<String>| items.join(", ");
        let type_items = named(types.iter().map(|binding| render(binding)).collect());
        let value_items = named(values.iter().map(|binding| render(binding)).collect());
        let rendered = if values.is_empty() {
            format!("{indent}import type {{ {type_items} }} from {quote}{request}{quote};")
        } else {
            format!(
                "{indent}import {{ {value_items} }} from {quote}{request}{quote};\n\
                 {indent}import type {{ {type_items} }} from {quote}{request}{quote};"
            )
        };
        edits.push((statement.start, statement.end, rendered));
    }
    apply_replacements(content, &edits)
}

/// Rewrite a dynamic `import('<spec>')` of a declared package onto the package's record url.
///
/// The reference does the same (`import("@normalized:N&&&@cxy/sandboxfinder/Index&1.0.5")`,
/// measured on clashbox, 2026-11-06). A bare package name in the abc names no record, so a
/// dynamic load would fail even though the package was compiled into the same abc.
fn rewrite_dynamic_package_imports(
    content: &str,
    packages: &[crate::build::deps::npm::Package],
) -> String {
    let mut edits: Vec<(usize, usize, String)> = Vec::new();
    for request in crate::build::deps::npm::dynamic_requests(content) {
        let Some(url) = package_url(&request.request, packages) else {
            continue;
        };
        let quote = &content[request.start..request.start + 1];
        edits.push((request.start, request.end, format!("{quote}{url}{quote}")));
    }
    apply_replacements(content, &edits)
}

/// Whether `target` names a directory whose `index` file is the real module.
fn package_entry_missing(package: &crate::build::deps::npm::Package, target: &str) -> bool {
    let base = package.source_root.join(target);
    for ext in ["ets", "ts", "js"] {
        if base.with_extension(ext).is_file() {
            return false;
        }
    }
    base.join("index.ets").is_file() || base.join("index.ts").is_file()
}

/// Join a package-relative import against the importing file's directory.
fn join_relative(dir: &std::path::Path, spec: &str) -> String {
    let mut parts: Vec<String> = dir
        .components()
        .map(|c| c.as_os_str().to_string_lossy().to_string())
        .collect();
    for seg in spec.split('/') {
        match seg {
            "." | "" => {}
            ".." => {
                parts.pop();
            }
            other => parts.push(other.to_string()),
        }
    }
    let joined = parts.join("/");
    for suffix in [".js", ".ts", ".ets"] {
        if let Some(stripped) = joined.strip_suffix(suffix) {
            return stripped.to_string();
        }
    }
    joined
}

/// A specifier inside a package that names the package itself or one of its modules.
fn bare_in_package(spec: &str, package: &crate::build::deps::npm::Package) -> Option<String> {
    if spec == package.name {
        return Some(String::new());
    }
    let rest = spec.strip_prefix(&format!("{}/", package.name))?;
    Some(
        rest.trim_end_matches(".js")
            .trim_end_matches(".ts")
            .trim_end_matches(".ets")
            .to_string(),
    )
}

/// Whether a package file uses ArkTS component syntax rather than plain TypeScript.
fn is_arkts(src: &str) -> bool {
    // A component library may carry whole files commented out; only live code decides how a
    // file has to be compiled.
    let live: String = src
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    [
        "@Component",
        "@Entry",
        "@ComponentV2",
        "@Extend",
        "@Styles",
        "@CustomDialog",
    ]
    .iter()
    .any(|marker| live.contains(marker))
}

/// Drop lines that are entirely a `//` comment, keeping the live code verbatim.
fn drop_live_comment_lines(src: &str) -> String {
    // Only the parked copy is dropped. A library that keeps an earlier revision of itself in
    // the header would otherwise have that revision read as the live component, and the copy
    // is always a leading run of comment lines — never a comment inside the live code.
    // Stripping every `//` line instead erased the library's own comments
    // (`fixtures/…` aside, measured on `@mcui/mccharts`: `Charts.ets` writes 45 comment lines
    // inside its methods and they all disappeared, while its header carries none).
    let lines: Vec<&str> = src.lines().collect();
    let mut leading = 0usize;
    while leading < lines.len() {
        let trimmed = lines[leading].trim();
        if trimmed.is_empty() || trimmed.starts_with("//") {
            leading += 1;
        } else {
            break;
        }
    }
    // A plain explanatory header (a licence, a note) is not a parked copy and stays. The copy
    // is recognised by what its lines spell: declarations and decorators.
    let looks_parked = lines[..leading].iter().any(|line| {
        let body = line.trim().trim_start_matches('/').trim();
        [
            "import ",
            "export ",
            "struct ",
            "class ",
            "interface ",
            "const ",
            "let ",
            "function ",
        ]
        .iter()
        .any(|keyword| body.starts_with(keyword))
            || body.starts_with('@')
    });
    let mut out = String::with_capacity(src.len());
    for (index, line) in lines.iter().enumerate() {
        if looks_parked && index < leading && line.trim_start().starts_with("//") {
            out.push('\n');
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

/// The build-profile record, relative to the module root.
///
/// hvigor writes it to `<module>/build/<product>/generated/profile/<target>/BuildProfile.ets`
/// (`build-profile-task.js` and `ModulePathInfoIml.getGenerateBuildProfilePath`, whose
/// `BuildArtifactConst.BUILD_PROFILE_FILE` is `BuildProfile.ets`); the staged tree carries the
/// `.ts` the compiler reads instead.
fn build_profile_record(product: &str, target: &str) -> String {
    format!("build/{product}/generated/profile/{target}/BuildProfile")
}

/// One exported value of the generated profile; hvigor rejects anything else
/// (`BUILDPROFILEFIELDS_ONLY_SUPPORT_NUMBER_STRING_BOOLEAN`).
enum ProfileValue<'a> {
    Text(&'a str),
    Number(u32),
    Boolean(bool),
}

/// The fields the profile exports, in hvigor's insertion order (`initDefaultData`).
#[allow(clippy::too_many_arguments)]
fn build_profile_fields<'a>(
    bundle: &'a str,
    bundle_type: &'a str,
    version_code: u32,
    version: &'a str,
    target: &'a str,
    product: &'a str,
    build_mode: &'a str,
    debug: bool,
    build_version: Option<&'a str>,
) -> Vec<(&'a str, ProfileValue<'a>)> {
    let mut fields = vec![
        ("BUNDLE_NAME", ProfileValue::Text(bundle)),
        ("BUNDLE_TYPE", ProfileValue::Text(bundle_type)),
        ("VERSION_CODE", ProfileValue::Number(version_code)),
        ("VERSION_NAME", ProfileValue::Text(version)),
        ("TARGET_NAME", ProfileValue::Text(target)),
        ("PRODUCT_NAME", ProfileValue::Text(product)),
        ("BUILD_MODE_NAME", ProfileValue::Text(build_mode)),
        ("DEBUG", ProfileValue::Boolean(debug)),
    ];
    // hvigor appends `BUILD_VERSION` only when the app declares one; an absent key drops the
    // export line instead of writing an empty string.
    if let Some(build_version) = build_version {
        fields.push(("BUILD_VERSION", ProfileValue::Text(build_version)));
    }
    fields
}

/// The generated profile module, byte for byte as hvigor's `normalizedFileData` prints it.
///
/// `build-profile-utils.js` joins a const block and a compatibility class with one newline:
/// the header comment, one `export const` per field (a string in single quotes, a number or a
/// boolean bare), a blank line, then `export default class BuildProfile { ` — the trailing
/// space is the template's — with one `\t`-indented `static readonly K = K;` per field and a
/// final `}` that carries no trailing newline.
///
/// The reference cache's copy is the same program after the ets-loader's printer (four spaces,
/// double quotes, the blank line collapsed); measured on clashbox, both forms compile to the
/// same abc (`lines 153/153`, `mnemonic_blocks 0`, `symbols 2/2`), so the raw hvigor form is
/// what this emits.
fn render_build_profile(fields: &[(&str, ProfileValue<'_>)]) -> String {
    let mut out = String::from(
        "/**\n * Use these variables when you tailor your ArkTS code. They must be of the const type.\n */\n",
    );
    for (name, value) in fields {
        match value {
            ProfileValue::Text(value) => {
                out.push_str(&format!("export const {name} = '{value}';\n"));
            }
            ProfileValue::Number(value) => {
                out.push_str(&format!("export const {name} = {value};\n"))
            }
            ProfileValue::Boolean(value) => {
                out.push_str(&format!("export const {name} = {value};\n"));
            }
        }
    }
    out.push('\n');
    out.push_str(
        "/**\n * BuildProfile Class is used only for compatibility purposes.\n */\nexport default class BuildProfile { \n",
    );
    for (name, _) in fields {
        out.push_str(&format!("\tstatic readonly {name} = {name};\n"));
    }
    out.push('}');
    out
}

/// Whether a compiled module imports the build-profile record.
///
/// `rewrite_spec` sends `import BuildProfile from 'BuildProfile'` to
/// `@normalized:N&&&<module>/<record>&<version>`; only a statement that keeps a value binding
/// is resolved that way, which is exactly the statement that makes the compiler pull the
/// record in (a type-only clause keeps the bare request and es2abc erases it). The question is
/// read off the module specifiers, never off the text: a string or a comment that spells the
/// path must not create a record.
fn imports_build_profile(content: &str, profile_record: &str) -> bool {
    crate::build::ets_transform::module_specifiers(content).is_some_and(|specifiers| {
        specifiers
            .iter()
            .any(|specifier| specifier.request.contains(profile_record))
    })
}

#[cfg(test)]
mod tests {
    use super::rewrite_relative_imports;

    /// The record the reference package carries for clashbox's entry product.
    const TEST_PROFILE_RECORD: &str = "build/default/generated/profile/default/BuildProfile";

    /// hvigor's raw `BuildProfile.ets` for clashbox (`entry/build/default/generated/profile/
    /// default/BuildProfile.ets`, 2026-11-07), read back field by field: tabs, the class head's
    /// trailing space, and a final `}` with no newline after it.
    #[test]
    fn the_profile_matches_hvigor_byte_for_byte() {
        let fields = super::build_profile_fields(
            "org.xbgroup.clashboxLTS",
            "app",
            1007048,
            "1.7.4-lts-stable.1",
            "default",
            "default",
            "debug",
            true,
            None,
        );
        let expected = concat!(
            "/**\n * Use these variables when you tailor your ArkTS code. They must be of the const type.\n */\n",
            "export const BUNDLE_NAME = 'org.xbgroup.clashboxLTS';\n",
            "export const BUNDLE_TYPE = 'app';\n",
            "export const VERSION_CODE = 1007048;\n",
            "export const VERSION_NAME = '1.7.4-lts-stable.1';\n",
            "export const TARGET_NAME = 'default';\n",
            "export const PRODUCT_NAME = 'default';\n",
            "export const BUILD_MODE_NAME = 'debug';\n",
            "export const DEBUG = true;\n",
            "\n",
            "/**\n * BuildProfile Class is used only for compatibility purposes.\n */\n",
            "export default class BuildProfile { \n",
            "\tstatic readonly BUNDLE_NAME = BUNDLE_NAME;\n",
            "\tstatic readonly BUNDLE_TYPE = BUNDLE_TYPE;\n",
            "\tstatic readonly VERSION_CODE = VERSION_CODE;\n",
            "\tstatic readonly VERSION_NAME = VERSION_NAME;\n",
            "\tstatic readonly TARGET_NAME = TARGET_NAME;\n",
            "\tstatic readonly PRODUCT_NAME = PRODUCT_NAME;\n",
            "\tstatic readonly BUILD_MODE_NAME = BUILD_MODE_NAME;\n",
            "\tstatic readonly DEBUG = DEBUG;\n",
            "}"
        );
        assert_eq!(super::render_build_profile(&fields), expected);
    }

    /// `BUILD_VERSION` comes last and disappears entirely when the app declares no
    /// `buildVersion` (clashbox's case: `app.json5` has none).
    #[test]
    fn build_version_is_optional() {
        let with = super::build_profile_fields(
            "b",
            "app",
            1,
            "1.0",
            "default",
            "default",
            "debug",
            true,
            Some("2"),
        );
        let text = super::render_build_profile(&with);
        assert!(
            text.contains("export const DEBUG = true;\nexport const BUILD_VERSION = '2';\n"),
            "{text}"
        );
        assert!(
            text.contains("\tstatic readonly BUILD_VERSION = BUILD_VERSION;\n}"),
            "{text}"
        );
        let without = super::build_profile_fields(
            "b", "app", 1, "1.0", "default", "default", "debug", true, None,
        );
        assert!(!super::render_build_profile(&without).contains("BUILD_VERSION"));
    }

    /// Only an import of the resolved record needs the profile: a type-only clause keeps the
    /// bare request, and a string or comment that spells the path is not a module request.
    #[test]
    fn only_an_import_of_the_record_needs_the_profile() {
        let content = "import BuildProfile from '@normalized:N&&&entry/build/default/generated/profile/default/BuildProfile&';\n\
                       if (BuildProfile.DEBUG) { console.log(1); }\n";
        assert!(super::imports_build_profile(content, TEST_PROFILE_RECORD));
        for unrelated in [
            "import type BuildProfile from 'BuildProfile';\nconst x = 1;\n",
            "const path = 'build/default/generated/profile/default/BuildProfile';\n",
            "// import BuildProfile from 'BuildProfile'\nconst x = 1;\n",
            "import other from '@normalized:N&&&entry/src/main/ets/entryability/Other&';\n",
        ] {
            assert!(
                !super::imports_build_profile(unrelated, TEST_PROFILE_RECORD),
                "{unrelated}"
            );
        }
    }

    /// A module tree under the system temp dir. Names are distinct per test: the test runner
    /// shares one process, so a fixed name would let two tests overwrite each other's sources.
    fn tree(name: &str, files: &[(&str, &str)]) -> std::path::PathBuf {
        let root =
            std::env::temp_dir().join(format!("hvolt-closure-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for (path, body) in files {
            let file = root.join(path);
            std::fs::create_dir_all(file.parent().expect("parent")).expect("mkdir");
            std::fs::write(&file, body).expect("write");
        }
        root
    }

    fn relative(root: &std::path::Path, files: &[std::path::PathBuf]) -> Vec<String> {
        let mut names: Vec<String> = files
            .iter()
            .map(|file| {
                file.strip_prefix(root)
                    .expect("relative")
                    .to_string_lossy()
                    .replace('\\', "/")
            })
            .collect();
        names.sort();
        names
    }

    /// The closure is the import graph from the entries: an import reaches a sibling with or
    /// without its extension and may climb with `..`; a bare specifier names no file of the
    /// tree and a source nothing imports carries no record.
    #[test]
    fn source_closure_walks_the_import_graph_from_the_entries() {
        let root = tree(
            "graph",
            &[
                ("EntryAbility.ets", "export default class EntryAbility {}\n"),
                (
                    "pages/Index.ets",
                    "import { helper } from './Helper';\nimport { deep } from './nested/Deep';\nimport { curves } from '@kit.ArkUI';\n",
                ),
                ("pages/Helper.ets", "export const helper = 1;\n"),
                (
                    "pages/nested/Deep.ets",
                    "import { unused } from '../../Unused';\nexport const deep = unused;\n",
                ),
                ("Unused.ets", "export const unused = 2;\n"),
                ("pages/Absent.ets", "export const absent = 3;\n"),
            ],
        );
        let entries = vec![
            "EntryAbility.ets".to_string(),
            "pages/Index.ets".to_string(),
        ];
        let (files, _) = super::source_closure(&root, &entries).expect("closure");
        assert_eq!(
            relative(&root, &files),
            vec![
                "EntryAbility.ets",
                "Unused.ets",
                "pages/Helper.ets",
                "pages/Index.ets",
                "pages/nested/Deep.ets",
            ],
            "{files:?}"
        );
    }

    /// A directory import names its `index` file, and an entry may be spelled with the `ets/`
    /// root `module.json` uses.
    #[test]
    fn source_closure_resolves_directory_imports_and_entry_prefixes() {
        let root = tree(
            "directory",
            &[
                ("Index.ets", "import { w } from './widgets';\n"),
                ("widgets/index.ets", "export const w = 1;\n"),
            ],
        );
        let entries = vec!["ets/Index.ets".to_string()];
        let (files, _) = super::source_closure(&root, &entries).expect("closure");
        assert_eq!(
            relative(&root, &files),
            vec!["Index.ets", "widgets/index.ets"]
        );
    }

    /// A root that names no file is reported with its path: compiling nothing would otherwise
    /// look like a successful build with no records.
    #[test]
    fn a_misplaced_entry_is_found_by_its_file_name() {
        // The ability descriptor may name a directory the file does not live in; the file name
        // still identifies it.
        let root = tree(
            "misplaced",
            &[("pages/Index.ets", ""), ("RootAbility.ets", "")],
        );
        let (files, staged_at) =
            super::source_closure(&root, &["backupability/RootAbility.ets".to_string()])
                .expect("misplaced entry resolves by name");
        assert!(
            files.iter().any(|file| file.ends_with("RootAbility.ets")),
            "{files:?}"
        );
        // The record name comes from the declared path, not from where the file happens to
        // sit: the device loads the ability through module.json's `srcEntry`.
        assert_eq!(
            staged_at
                .get(&root.join("RootAbility.ets"))
                .map(|path| path.to_string_lossy().replace('\\', "/")),
            Some("backupability/RootAbility.ets".to_string()),
            "{staged_at:?}"
        );
    }

    #[test]
    fn source_closure_reports_an_entry_that_names_no_file() {
        let root = tree("missing", &[("pages/Index.ets", "")]);
        let err = super::source_closure(&root, &["pages/Gone.ets".to_string()])
            .expect_err("missing entry");
        assert!(err.contains("pages/Gone.ets"), "{err}");
    }
    use std::path::Path;

    /// A specifier reaches the abc as a normalized url: a bare `./X` names no record.
    #[test]
    fn multiline_and_quoted_imports_are_rewritten() {
        let src = "import {\n  a,\n  b,\n} from \"./About\"\nimport x from './Sub/Y'\n";
        let out = rewrite_relative_imports(
            src,
            "entry",
            "entry",
            Path::new("components/Settings/Settings.ts"),
            &[],
            &[],
            &[],
            TEST_PROFILE_RECORD,
        );
        assert!(
            out.contains("@normalized:N&&&entry/src/main/ets/components/Settings/About&"),
            "{out}"
        );
        assert!(
            out.contains("@normalized:N&&&entry/src/main/ets/components/Settings/Sub/Y&"),
            "{out}"
        );
        assert!(!out.contains("\"./About\""), "{out}");
        assert!(!out.contains("'./Sub/Y'"), "{out}");
    }

    /// The exact urls the reference emits for the same sources (`fixtures/import-relative`
    /// golden): the module root, the source path and the entry module's empty version.
    #[test]
    fn relative_import_url_matches_the_reference() {
        let src = "import { helper } from './Helper';\nimport { deep } from './nested/Deep';\n";
        let out = rewrite_relative_imports(
            src,
            "entry",
            "entry",
            Path::new("pages/Index.ts"),
            &[],
            &[],
            &[],
            TEST_PROFILE_RECORD,
        );
        assert_eq!(
            out,
            "import { helper } from '@normalized:N&&&entry/src/main/ets/pages/Helper&';\n\
             import { deep } from '@normalized:N&&&entry/src/main/ets/pages/nested/Deep&';\n"
        );
    }

    /// A package file's requests resolve per request, exactly like a module's: a statement
    /// that keeps a value binding loads the request, and every statement that names it takes
    /// the package url, the type-only copy included (`proxy_core/Index.ets`). A request
    /// nothing loads keeps the path as written (`@cxy/sandboxfinder`'s
    /// `import type { FileManager } from '../manager/FileManager'`).
    #[test]
    fn a_package_request_is_resolved_where_any_statement_loads_it() {
        let package = crate::build::deps::npm::Package {
            name: "@test/pkg".to_string(),
            alias: "@test/pkg".to_string(),
            dir: std::path::PathBuf::from("/p"),
            version: "1.0.0".to_string(),
            source_root: std::path::PathBuf::from("/p"),
            entry: "Index".to_string(),
            kind: crate::build::deps::npm::Kind::Arkts,
        };
        let content = "import { Helper } from '../utils/Helper';\n\
                       import type { Shape } from '../utils/Helper';\n\
                       import type { Local } from './Local';\n";
        let out = super::rewrite_package_imports(content, &package, Path::new("server/Api.ts"));
        assert_eq!(
            out,
            "import { Helper } from '@normalized:N&&&@test/pkg/utils/Helper&1.0.0';\n\
             import type { Shape } from '@normalized:N&&&@test/pkg/utils/Helper&1.0.0';\n\
             import type { Local } from './Local';\n"
        );
    }

    /// A sibling module's files carry its own version; the entry module carries none
    /// (`abc_compile::record_version`).
    #[test]
    fn sibling_module_urls_carry_the_module_version() {
        let members = ["entry".to_string(), "proxy_core".to_string()];
        let out = rewrite_relative_imports(
            "import x from 'proxy_core';\nimport y from 'proxy_core/Utils';\n",
            "entry",
            "entry",
            Path::new("pages/Index.ts"),
            &[],
            &members,
            &[],
            TEST_PROFILE_RECORD,
        );
        // The bare request names the member module's package entry, whose record is
        // `&proxy_core/Index&1.0.0` in the reference's own filesInfo.txt.
        assert!(
            out.contains("'@normalized:N&&&proxy_core/Index&1.0.0'"),
            "{out}"
        );
        assert!(
            out.contains("'@normalized:N&&&proxy_core/Utils&1.0.0'"),
            "{out}"
        );
    }

    /// A package that ships only declarations keeps its components behind its entry: the walk
    /// follows the entry's re-exports and nothing else, so a declaration the package does not
    /// export is never read. (`@linke/ui-color-picker` is exactly this shape: `ets/modules.abc`
    /// plus `.d.ets`, with `Index.d.ets` re-exporting one file per component.)
    #[test]
    fn a_declaration_only_package_is_read_through_its_entry() {
        let root = tree(
            "dets",
            &[
                ("Index.d.ets", "export { Dialog } from './src/Dialog';\n"),
                (
                    "src/Dialog.d.ets",
                    "@CustomDialog\n@Component\nexport declare struct Dialog {\n    @Link\n    title: string;\n}\n",
                ),
                (
                    "src/Unused.d.ets",
                    "@Component\nexport declare struct Unused {}\n",
                ),
            ],
        );
        let package = crate::build::deps::npm::Package {
            name: "probe".to_string(),
            alias: "probe".to_string(),
            dir: root.clone(),
            version: "1.0.0".to_string(),
            source_root: root.clone(),
            entry: "Index".to_string(),
            kind: crate::build::deps::npm::Kind::ByteCode,
        };
        assert_eq!(
            relative(&root, &super::package_declaration_files(&package)),
            vec!["Index.d.ets", "src/Dialog.d.ets"]
        );
    }

    /// A kit's table states a module the way the output spells it (`@ohos:…`), while a source
    /// may import the module itself (`@ohos.…`); both name the same declaration file, and a
    /// module the SDK ships no ArkTS declaration for names none.
    #[test]
    fn a_system_module_request_names_its_declaration_file() {
        let root = tree(
            "sdk-api",
            &[(
                "api/@ohos.arkui.advanced.SegmentButton.d.ets",
                "@Component\ndeclare struct SegmentButton {\n    @ObjectLink\n    options: number;\n}\n",
            )],
        );
        let api = vec![root.join("api")];
        let expected = Some(root.join("api/@ohos.arkui.advanced.SegmentButton.d.ets"));
        for request in [
            "@ohos:arkui.advanced.SegmentButton",
            "@ohos.arkui.advanced.SegmentButton",
        ] {
            assert_eq!(
                super::sdk_declaration_file(&api, request),
                expected,
                "{request}"
            );
        }
        assert_eq!(
            super::sdk_declaration_file(&api, "@ohos:nothing.Here"),
            None
        );
        assert_eq!(super::sdk_declaration_file(&api, "yaml"), None);
    }

    /// A file's coordinate is its staged path with the source extension: a module's tree is flat,
    /// so the source root is put back, while a package is staged under `oh_modules/.ohpm` where
    /// the reference compiles it (`process_component_member.js::componentParamRowAndColumn`
    /// writes `path.relative(cwd, resourceFileName)`).
    #[test]
    fn a_source_coordinate_names_the_staged_path_with_its_source_extension() {
        assert_eq!(
            super::source_coordinate(
                "xb_components/src/main/ets",
                Path::new("view/Xb_Studio.ets")
            ),
            "xb_components/src/main/ets/view/Xb_Studio.ets"
        );
        assert_eq!(
            super::source_coordinate(
                "oh_modules/.ohpm/@mcui+mccharts@2.8.9/oh_modules/@mcui/mccharts",
                Path::new("src/main/ets/components/mainpage/McLineChart.ets"),
            ),
            "oh_modules/.ohpm/@mcui+mccharts@2.8.9/oh_modules/@mcui/mccharts/src/main/ets/components/mainpage/McLineChart.ets"
        );
    }

    /// A dependency source is not a page and has no route to name it, so the staging loop names
    /// it: without that, the file carries the page the thread lowered before it, which is what
    /// `@mcui/mccharts`' five charts did (measured on clashbox, 2026-11-07).
    #[test]
    fn a_package_source_is_named_by_its_own_staged_path() {
        let root = tree(
            "package-page",
            &[(
                "src/main/ets/Nav.ets",
                "@Component\nstruct Nav {\n  build() {\n    Navigation() {\n      NavDestination() {\n        Text('x')\n      }\n    }\n  }\n}\n",
            )],
        );
        let package = crate::build::deps::npm::Package {
            name: "@mcui/mccharts".to_string(),
            alias: "@mcui/mccharts".to_string(),
            dir: root.clone(),
            version: "2.8.9".to_string(),
            source_root: root.join("src/main/ets"),
            entry: "Nav".to_string(),
            kind: crate::build::deps::npm::Kind::Arkts,
        };
        let out = std::env::temp_dir().join(format!("hvolt-package-page-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&out);
        std::fs::create_dir_all(&out).expect("create the staging tree");
        // The coordinate the page before this file left behind.
        crate::build::ets_compile::set_source_page("xb_components/src/main/ets/view/Xb_Studio.ets");
        super::stage_package(
            &package,
            &out,
            &[],
            &[],
            &Default::default(),
            &Default::default(),
        )
        .expect("stage the package");
        let coordinate =
            "oh_modules/.ohpm/@mcui+mccharts@2.8.9/oh_modules/@mcui/mccharts/src/main/ets/Nav.ets";
        assert_eq!(
            crate::build::ets_compile::source_page().as_deref(),
            Some(coordinate)
        );
        // The route object the file emits reads the same coordinate back.
        let staged = std::fs::read_to_string(out.join(package.staging_dir()).join("Nav.ts"))
            .expect("the staged source");
        assert!(staged.contains("pagePath:"), "{staged}");
        assert!(
            staged.contains(
                "oh_modules/.ohpm/@mcui+mccharts@2.8.9/oh_modules/@mcui/mccharts/src/main/ets/Nav"
            ),
            "{staged}"
        );
        assert!(!staged.contains("Xb_Studio"), "{staged}");
    }

    /// The owner argument belongs to the construction, not to the builder: a controller whose
    /// `builder` names a method carries `this` in the reference too
    /// (`Xb_SettingItem`: `builder: this.ColorPickerDialog` → `}, this);`), and the pass that
    /// rewrites component builders never sees that site.
    #[test]
    fn a_method_builder_controller_still_takes_its_owner() {
        let content = "class P extends PUV2ViewBase {\n    aboutToAppear() {\n        this.controller = new CustomDialogController({\n            builder: this.PickerDialog,\n            autoCancel: false\n        });\n    }\n}\n";
        let out = super::rewrite_dialog_builders(content, content);
        assert!(
            out.contains("autoCancel: false\n        }, this);"),
            "{out}"
        );
        assert_eq!(out.matches(", this)").count(), 1, "{out}");
    }
}

#[cfg(test)]
mod type_import_tests {
    use super::declares_as_type;

    /// An enum is a value even when the module only declares it: the emitted code reads its
    /// members, so the binding has to survive the type-only pass (`@ohos:multimodalInput.keyCode`
    /// declares `KeyCode` this way, and clashbox reads `KeyCode.KEYCODE_ESCAPE`).
    #[test]
    fn a_declared_enum_with_runtime_constants_is_a_value() {
        let src =
            "export declare enum KeyCode {\n  KEYCODE_ESCAPE = 2070,\n  KEYCODE_F1 = 2032,\n}\n";
        assert!(!declares_as_type(src, "KeyCode"));
        let strings = "declare enum Mode {\n  Off = 'off',\n}\n";
        assert!(!declares_as_type(strings, "Mode"));
        // A plain interface is still a type.
        assert!(declares_as_type(
            "export declare interface Legacy {\n}\n",
            "Legacy"
        ));
    }

    #[test]
    fn interface_names_are_recognised() {
        let src = "export interface ProxyGroup{\n}\nexport class Keep {}\n";
        assert!(declares_as_type(src, "ProxyGroup"));
        assert!(!declares_as_type(src, "Keep"));
    }

    /// The declaration is read from its node: `export`, `declare` and the alias form all
    /// count, while an `export { type X }` specifier only re-exports and declares nothing.
    #[test]
    fn declared_type_forms_are_read_from_the_tree() {
        assert!(declares_as_type("export type Id = string;\n", "Id"));
        assert!(declares_as_type(
            "export declare interface Legacy {\n}\n",
            "Legacy"
        ));
        assert!(!declares_as_type(
            "export { type Remote } from './m';\n",
            "Remote"
        ));
    }

    /// A declaration list with no bodies is how a sibling module's types are summarised; the
    /// parser rejects it, and the token scan still reads the names.
    #[test]
    fn a_target_that_does_not_parse_falls_back_to_its_tokens() {
        assert!(declares_as_type("interface Group\ntype Mode\n", "Group"));
        assert!(declares_as_type("interface Group\ntype Mode\n", "Mode"));
    }
}

#[cfg(test)]
mod package_specifier_tests {
    use super::package_specifiers;

    /// 只收非相对包名：相对导入由项目表回答，SDK 名字由 SDK 那一路回答。
    #[test]
    fn only_bare_package_names_are_collected() {
        let source = "import { a } from './a';\nimport type { B } from '../b';\nimport { c } from '@demo/kinds';\nimport 'side-effect-pkg';\nexport const x = a;\n";
        assert_eq!(
            package_specifiers(source),
            vec!["@demo/kinds".to_string(), "side-effect-pkg".to_string()]
        );
    }

    /// 同一个包被多次导入只出现一次。
    #[test]
    fn a_package_is_named_once() {
        let source = "import { a } from '@demo/kinds';\nimport { b } from '@demo/kinds';\n";
        assert_eq!(package_specifiers(source), vec!["@demo/kinds".to_string()]);
    }
}

#[cfg(test)]
mod shared_module_tests {
    use super::*;

    #[test]
    fn a_leading_directive_marks_the_file() {
        assert!(is_shared_module(
            "\"use shared\"\n\nexport function f(): void {}\n"
        ));
    }

    #[test]
    fn imports_may_precede_the_directive() {
        let source = "import { a } from './a';\nimport type { B } from './b';\n\"use shared\"\nexport const x = a;\n";
        assert!(is_shared_module(source));
    }

    #[test]
    fn a_single_quoted_directive_counts_too() {
        assert!(is_shared_module("'use shared'\nexport {};\n"));
    }

    /// The reference stops at the first non-import statement: a directive written after one is
    /// an ordinary expression statement, not a module marker.
    #[test]
    fn a_directive_after_another_statement_does_not_count() {
        assert!(!is_shared_module(
            "export const first = 1;\n\"use shared\"\n"
        ));
    }

    #[test]
    fn a_file_without_the_directive_is_not_shared() {
        assert!(!is_shared_module("@Entry\n@Component\nstruct A {}\n"));
        assert!(!is_shared_module(""));
    }
}

/// The source-map entry for one staged file.
///
/// The path in the key is the module-relative path of the emitted `.ts`: hvolt's own staging is
/// flat, but the map speaks the module layout every tool reads (`<module>/src/main/ets/...`), and
/// the entry ability lives under `entryability/` there. Only files hvolt actually compiles are
/// recorded — a map entry is a claim that the file exists in the build.
fn source_map_entry(
    module: &str,
    entry_file: &str,
    rel: &std::path::Path,
    source: &str,
    content: &str,
    is_component: bool,
    project_builders: &[String],
) -> crate::build::compile::ets_source_map::MapEntry {
    let stem = rel
        .file_stem()
        .map(|stem| stem.to_string_lossy().to_string())
        .unwrap_or_default();
    let ability = entry_file.trim_start_matches("./");
    let module_path = if rel == std::path::Path::new(ability) {
        format!("src/main/ets/entryability/{stem}.ts")
    } else {
        format!(
            "src/main/ets/{}.ts",
            rel.with_extension("").to_string_lossy().replace('\\', "/")
        )
    };
    let anchors = if is_component {
        match crate::build::lower::ets_component::parse_components(source, project_builders) {
            Ok(components) => {
                let positions: Vec<_> = components
                    .iter()
                    .map(|component| component.source_positions())
                    .collect();
                crate::build::compile::ets_source_map::component_anchors(
                    source, content, &positions,
                )
            }
            Err(_) => crate::build::compile::ets_source_map::plain_anchors(source, content),
        }
    } else {
        crate::build::compile::ets_source_map::plain_anchors(source, content)
    };
    crate::build::compile::ets_source_map::MapEntry {
        module: module.to_string(),
        module_path: module_path.clone(),
        file: format!("{stem}.ets"),
        source: format!("{module}/{}.ets", module_path.trim_end_matches(".ts")),
        anchors,
    }
}

/// Write `sourceMaps.map` where the reference keeps it: once in the module build directory
/// (the debug copy next to `modules.abc`) and once in the packaged output.
fn write_source_maps(
    out_abc: &std::path::Path,
    modules: &[StageModule<'_>],
    version: &str,
    entries: &[crate::build::compile::ets_source_map::MapEntry],
) -> Result<(), String> {
    if entries.is_empty() {
        return Ok(());
    }
    let entry_package = modules.first().map(|module| module.name).unwrap_or("");
    let document = crate::build::compile::ets_source_map::document(entry_package, version, entries);
    let text =
        serde_json::to_string_pretty(&document).map_err(|e| format!("sourceMaps.map: {e}"))?;
    let ets_dir = out_abc
        .parent()
        .ok_or_else(|| "sourceMaps.map: no module build directory".to_string())?;
    std::fs::write(ets_dir.join("sourceMaps.map"), &text)
        .map_err(|e| format!("write sourceMaps.map: {e}"))?;
    let packaged = ets_dir.join("../outputs/default/mapping");
    std::fs::create_dir_all(&packaged).map_err(|e| format!("mkdir {}: {e}", packaged.display()))?;
    std::fs::write(packaged.join("sourceMaps.map"), &text)
        .map_err(|e| format!("write packaged sourceMaps.map: {e}"))?;
    Ok(())
}

#[cfg(test)]
mod dialog_builder_tests {
    use super::{dialog_controllers, line_indent, source_call_positions};

    /// The controller is found by its construction, and the field it is stored in comes from
    /// the assignment around it.
    #[test]
    fn a_controller_is_read_with_the_field_it_is_assigned_to() {
        let content = "class P extends ViewPU {\n    constructor() {\n        this.dialog = new CustomDialogController({ builder: () => { } });\n    }\n}\n";
        let found = dialog_controllers(content);
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].field.as_deref(), Some("dialog"));
        assert!(!found[0].has_owner);
    }

    /// A construction that already names an owner keeps its argument list.
    #[test]
    fn an_owner_already_written_is_not_repeated() {
        let content = "this.dialog = new CustomDialogController({ builder: () => { } }, this);\n";
        let found = dialog_controllers(content);
        assert_eq!(found.len(), 1);
        assert!(found[0].has_owner);
    }

    /// The coordinate names the call in the source file, not in the lowered text.
    #[test]
    fn a_call_is_located_by_its_token_sequence() {
        let source = "@Component\nstruct P {\n  build() {\n    Column() {\n      Child({ x: 1 })\n    }\n  }\n}\n";
        assert_eq!(
            source_call_positions(source, "Child", "{ x: 1 }"),
            vec![(5, 7)]
        );
    }

    /// Two identical calls keep their own coordinates, in source order: the caller hands the
    /// n-th construction the n-th occurrence.
    #[test]
    fn an_identical_call_keeps_its_own_position() {
        let source = "A({ x: 1 })\nA({ x: 1 })\n";
        assert_eq!(
            source_call_positions(source, "A", "{ x: 1 }"),
            vec![(1, 1), (2, 1)]
        );
    }

    /// A call the source does not repeat has no coordinate rather than a wrong one.
    #[test]
    fn an_absent_call_has_no_position() {
        assert!(source_call_positions("const a = 1;\n", "Child", "{ }").is_empty());
    }

    /// The printer writes the statement separators the source left to automatic semicolon
    /// insertion, so a `;` in the lowered arguments is not part of the call's shape.
    #[test]
    fn a_statement_separator_does_not_hide_the_call() {
        let source = "struct P {\n  build() {\n    Child({ center: () => {\n      this.x = 1\n    } })\n  }\n}\n";
        assert_eq!(
            source_call_positions(
                source,
                "Child",
                "{ center: () => {\n      this.x = 1;\n    } }"
            ),
            vec![(3, 5)]
        );
    }

    #[test]
    fn the_line_indent_is_the_leading_whitespace_of_the_line() {
        let text = "a\n\t\tthis.x = 1;\n";
        assert_eq!(line_indent(text, 4), "\t\t");
        assert_eq!(line_indent("plain", 0), "");
    }
}
