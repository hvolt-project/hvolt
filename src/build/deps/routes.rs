//! The routes a module registers: from `hvolt.toml`, or from its own `route_map.json`.
//!
//! A migrated project carries its routes in `hvolt.toml` and the packager generates
//! `resources/base/profile/route_map.json` from them; a project that has not migrated still
//! keeps the manifest on disk. Both shapes are read here so the consumers never disagree:
//!
//! - `compile_roots` needs the pages, or a page only reachable through `pushPathByName` never
//!   enters the merged abc;
//! - the lowering's route table needs `(name, page, buildFunction)`, or `navigation_init`
//!   generates no `NavigationBuilderRegister` and the device reports
//!   `can't find name in config file` (measured on clashbox, 2026-11-08).
//!
//! `page` and `build_function` may be omitted in the table: they are then derived from the
//! module's sources by matching the route name against each file's `@Builder export function`
//! (ignoring case) and its file stem. `hvolt import` omits both for the thirteen clashbox
//! routes whose stem and builder agree with the name; it writes `page` alone for `LogDetails`,
//! `build_function` alone for `EditArrayConfig`, and both for `route-address`, `compatible`,
//! `access_control` and `online_app_search`, whose builders share no lexical relation with
//! the names.

use std::path::Path;

/// One route, with the page resolved to a module-relative path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Route {
    pub(crate) name: String,
    /// Module-relative, e.g. `components/More/Log.ets`.
    pub(crate) page: String,
    pub(crate) build_function: String,
}

/// Every route a module registers.
///
/// The table wins when it is non-empty. Otherwise the module's own `route_map.json` is read —
/// a project that has not been migrated keeps one, and silently dropping its routes would make
/// every `NavPathStack` push fail at run time.
pub(crate) fn route_entries(
    module: Option<&crate::support::config::ModuleInfo>,
    module_dir: &Path,
) -> Result<Vec<Route>, String> {
    if let Some(module) = module.filter(|module| !module.router_map.is_empty()) {
        let builders = builder_functions(module_dir);
        let mut routes = Vec::with_capacity(module.router_map.len());
        for def in &module.router_map {
            // Both fields must come from the same file: a page from one file and a builder
            // from another resolves to a record the lowering never registers.
            let pair = resolve_route(def, &builders).ok_or_else(|| {
                // Name what is missing instead of blaming fields the table already has.
                let mut hint = Vec::new();
                if def.page.is_none() {
                    hint.push("`page`");
                }
                if def.build_function.is_none() {
                    hint.push("`build_function`");
                }
                format!(
                    "[[module.router_map]] `{}` could not be resolved: {}. Either add the \
                     missing field(s), or make the route name unambiguous within one file.",
                    def.name,
                    if hint.is_empty() {
                        "the fields it does have do not agree on one file".to_string()
                    } else {
                        format!(
                            "no builder or file matches, and it omits {}",
                            hint.join(" / ")
                        )
                    }
                )
            })?;
            routes.push(pair);
        }
        return Ok(routes);
    }
    Ok(manifest_entries(module_dir))
}

/// Resolve one table entry against the module's `@Builder` functions.
///
/// A route names one page and one builder. Every branch below refuses rather than guesses:
/// a file can hold several global builders (`Log.ets` declares `LogBuilder` and
/// `LogDetailsBuilder`), so "the file's first builder" is not an answer. Refusing makes the
/// build stop with the entry named; guessing registers a `NavigationBuilderRegister` that
/// opens the wrong page and reports nothing.
fn resolve_route(
    def: &crate::support::config::RouterMapDef,
    builders: &[(String, String)],
) -> Option<Route> {
    match (&def.page, &def.build_function) {
        (Some(page), explicit) => resolve_with_page(def, page, explicit.as_deref(), builders),
        (None, Some(explicit)) => resolve_with_builder(def, explicit, builders),
        (None, None) => resolve_by_name(def, builders),
    }
}

/// The page is written out: it pins the file, and the builder must live there.
fn resolve_with_page(
    def: &crate::support::config::RouterMapDef,
    page: &str,
    explicit: Option<&str>,
    builders: &[(String, String)],
) -> Option<Route> {
    let in_file: Vec<&String> = builders
        .iter()
        .filter(|(_, file)| file == page)
        .map(|(function, _)| function)
        .collect();
    let build_function = match explicit {
        Some(explicit) => {
            if !in_file.iter().any(|function| function.as_str() == explicit) {
                return None;
            }
            explicit.to_string()
        }
        None => pick_in_file(in_file, &def.name)?,
    };
    Some(Route {
        name: def.name.clone(),
        page: page.to_string(),
        build_function,
    })
}

/// No page written, but the builder is: it must name exactly one file.
fn resolve_with_builder(
    def: &crate::support::config::RouterMapDef,
    explicit: &str,
    builders: &[(String, String)],
) -> Option<Route> {
    let hits: Vec<&(String, String)> = builders
        .iter()
        .filter(|(function, _)| function == explicit)
        .collect();
    let (function, page) = match hits.as_slice() {
        [hit] => (*hit).clone(),
        _ => return None,
    };
    Some(Route {
        name: def.name.clone(),
        page,
        build_function: function,
    })
}

/// Neither field written: the name has to identify one builder and one file.
fn resolve_by_name(
    def: &crate::support::config::RouterMapDef,
    builders: &[(String, String)],
) -> Option<Route> {
    let by_name: Vec<&(String, String)> = builders
        .iter()
        .filter(|(function, _)| {
            let base = function.strip_suffix("Builder").unwrap_or(function);
            base.eq_ignore_ascii_case(&def.name)
        })
        .collect();
    match by_name.as_slice() {
        [only] => {
            return Some(Route {
                name: def.name.clone(),
                page: only.1.clone(),
                build_function: only.0.clone(),
            });
        }
        [] => {}
        // Two builders answer to the same name: the entry is ambiguous.
        _ => return None,
    }

    // Only a file stem matches. Take it when the file is unique and holds one builder; a file
    // with several builders is exactly the case this function must not guess at.
    let mut pages: Vec<&String> = builders
        .iter()
        .filter(|(_, file)| page_stem(file).eq_ignore_ascii_case(&def.name))
        .map(|(_, file)| file)
        .collect();
    pages.sort();
    pages.dedup();
    let [page] = pages.as_slice() else {
        return None;
    };
    let page = (*page).clone();
    let in_file: Vec<&String> = builders
        .iter()
        .filter(|(_, file)| file == &page)
        .map(|(function, _)| function)
        .collect();
    let build_function = pick_in_file(in_file, &def.name)?;
    Some(Route {
        name: def.name.clone(),
        page,
        build_function,
    })
}

/// Pick the builder of one page: one whose name matches, else the file's only builder.
fn pick_in_file(in_file: Vec<&String>, route_name: &str) -> Option<String> {
    let named: Vec<&&String> = in_file
        .iter()
        .filter(|function| {
            let base = function.strip_suffix("Builder").unwrap_or(function);
            base.eq_ignore_ascii_case(route_name)
        })
        .collect();
    match named.len() {
        1 => Some((*named[0]).clone()),
        0 if in_file.len() == 1 => Some(in_file[0].clone()),
        _ => None,
    }
}

/// A module's `hvolt.toml`, read from the module directory itself.
///
/// The caller must pass the directory, not the declared name: a staged directory can be
/// `src` while the module calls itself `entry`.
pub(crate) fn module_config(
    module_dir: &Path,
) -> Result<crate::support::config::ModuleConfig, String> {
    crate::support::config::ModuleConfig::from_file(&module_dir.join("hvolt.toml"))
}

/// The routes a module's own `route_map.json` declares, if it still has one.
fn manifest_entries(module_dir: &Path) -> Vec<Route> {
    for rel in [
        "resources/base/profile/route_map.json",
        "src/main/resources/base/profile/route_map.json",
    ] {
        let Ok(text) = std::fs::read_to_string(module_dir.join(rel)) else {
            continue;
        };
        let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) else {
            continue;
        };
        let entries: Vec<Route> = json["routerMap"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|item| {
                let source = item["pageSourceFile"].as_str()?;
                Some(Route {
                    name: item["name"].as_str().unwrap_or_default().to_string(),
                    page: source
                        .strip_prefix("src/main/ets/")
                        .unwrap_or(source)
                        .to_string(),
                    build_function: item["buildFunction"].as_str()?.to_string(),
                })
            })
            .collect();
        if !entries.is_empty() {
            return entries;
        }
    }
    Vec::new()
}

/// Every `@Builder` function the module declares: `(name, module-relative page)`.
fn builder_functions(module_dir: &Path) -> Vec<(String, String)> {
    let mut found = Vec::new();
    let mut stack = vec![module_dir.to_path_buf()];
    while let Some(dir) = stack.pop() {
        // Skipping a directory silently would make the builders it holds look absent, and
        // "absent" is what makes a name look unique below.
        let entries = match std::fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(error) => {
                eprintln!("⚠ cannot read {}: {error}", dir.display());
                continue;
            }
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().is_none_or(|ext| ext != "ets") {
                continue;
            }
            let source = match std::fs::read_to_string(&path) {
                Ok(source) => source,
                Err(error) => {
                    eprintln!("⚠ cannot read {}: {error}", path.display());
                    continue;
                }
            };
            let relative = path
                .strip_prefix(module_dir)
                .map(|rel| rel.to_string_lossy().replace('\\', "/"))
                .unwrap_or_else(|_| path.to_string_lossy().to_string());
            for name in builder_names(&source) {
                found.push((name, relative.clone()));
            }
        }
    }
    found.sort();
    found
}

/// The `@Builder` / `@LocalBuilder` function names in one source.
fn builder_names(source: &str) -> Vec<String> {
    use crate::build::lex::{tokenize, Kind};
    let tokens: Vec<_> = tokenize(source)
        .into_iter()
        .filter(|token| token.kind != Kind::Comment)
        .collect();
    let mut names = Vec::new();
    for (index, token) in tokens.iter().enumerate() {
        if token.kind != Kind::Ident || index == 0 || !tokens[index - 1].is_punct('@') {
            continue;
        }
        if token.text != "Builder" && token.text != "LocalBuilder" {
            continue;
        }
        let mut cursor = index + 1;
        if tokens.get(cursor).is_some_and(|next| next.text == "export") {
            cursor += 1;
        }
        if tokens
            .get(cursor)
            .is_none_or(|next| next.text != "function")
        {
            continue;
        }
        if let Some(name) = tokens.get(cursor + 1) {
            if name.kind == Kind::Ident {
                names.push(name.text.to_string());
            }
        }
    }
    names
}

fn page_stem(page: &str) -> &str {
    page.rsplit('/')
        .next()
        .and_then(|file| file.strip_suffix(".ets"))
        .unwrap_or(page)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::support::config::RouterMapDef;

    fn module_with(routes: Vec<RouterMapDef>) -> crate::support::config::ModuleInfo {
        let mut module = crate::support::config::ModuleConfig::default_entry().module;
        module.router_map = routes;
        module
    }

    /// A route whose page is written out is used as written, builder derived from the name.
    #[test]
    fn an_explicit_page_is_kept_and_the_builder_derived() {
        let dir = std::env::temp_dir().join(format!("hvolt-routes-{}", std::process::id()));
        let page = dir.join("components/Log.ets");
        std::fs::create_dir_all(page.parent().unwrap()).expect("mkdir");
        std::fs::write(&page, "@Builder\nexport function LogDetailsBuilder() {}\n").expect("write");

        let routes = route_entries(
            Some(&module_with(vec![RouterMapDef {
                name: "LogDetails".to_string(),
                page: Some("components/Log.ets".to_string()),
                build_function: None,
            }])),
            &dir,
        )
        .expect("resolves");
        assert_eq!(routes.len(), 1);
        assert_eq!(routes[0].build_function, "LogDetailsBuilder");
        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    /// A route that names both explicitly resolves when the builder is really in that page.
    #[test]
    fn an_explicit_route_needs_no_derivation() {
        let dir = std::env::temp_dir().join(format!("hvolt-routes2-{}", std::process::id()));
        let page = dir.join("components/Network.ets");
        std::fs::create_dir_all(page.parent().unwrap()).expect("mkdir");
        std::fs::write(
            &page,
            "@Builder\nexport function NetworkRouterAddressBuilder() {}\n",
        )
        .expect("write");
        let routes = route_entries(
            Some(&module_with(vec![RouterMapDef {
                name: "route-address".to_string(),
                page: Some("components/Network.ets".to_string()),
                build_function: Some("NetworkRouterAddressBuilder".to_string()),
            }])),
            &dir,
        )
        .expect("resolves");
        assert_eq!(
            routes,
            vec![Route {
                name: "route-address".to_string(),
                page: "components/Network.ets".to_string(),
                build_function: "NetworkRouterAddressBuilder".to_string(),
            }]
        );
        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    /// A module that has not migrated still contributes the routes in its own manifest.
    #[test]
    fn a_project_without_a_table_still_reads_its_manifest() {
        let dir = std::env::temp_dir().join(format!("hvolt-routes3-{}", std::process::id()));
        let profile = dir.join("resources/base/profile");
        std::fs::create_dir_all(&profile).expect("mkdir");
        std::fs::write(
            profile.join("route_map.json"),
            r#"{"routerMap":[{"name":"A","pageSourceFile":"src/main/ets/components/A.ets","buildFunction":"ABuilder"}]}"#,
        )
        .expect("write");
        let routes = route_entries(Some(&module_with(Vec::new())), &dir).expect("resolves");
        assert_eq!(routes.len(), 1);
        assert_eq!(routes[0].page, "components/A.ets");
        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    /// The config is read from the module directory, not from the declared name.
    ///
    /// A staged directory can be called `src` while the module calls itself `entry`; looking
    /// the config up by name would miss it and publish no routes at all.
    #[test]
    fn module_config_reads_from_the_directory() {
        let root = std::env::temp_dir().join(format!("hvolt-cfg-{}", std::process::id()));
        let module_dir = root.join("src");
        std::fs::create_dir_all(&module_dir).expect("mkdir");
        std::fs::write(
            module_dir.join("hvolt.toml"),
            "[module]\nname = \"entry\"\nentry = \"EntryAbility.ets\"\ndevice_types = [\"phone\"]\n",
        )
        .expect("write");
        let cfg = module_config(&module_dir).expect("the config is where the directory says");
        assert_eq!(cfg.module.name, "entry");
        std::fs::remove_dir_all(&root).expect("cleanup");
    }

    /// A file with two builders must pair the route by name, not take the first.
    ///
    /// `Log.ets` declares both `LogBuilder` and `LogDetailsBuilder`; taking the file's first
    /// builder registered `LogDetails` against `LogBuilder` and opened the wrong page.
    #[test]
    fn a_file_with_two_builders_pairs_by_name() {
        let dir = std::env::temp_dir().join(format!("hvolt-routes4-{}", std::process::id()));
        let page = dir.join("components/More/Log.ets");
        std::fs::create_dir_all(page.parent().unwrap()).expect("mkdir");
        std::fs::write(
            &page,
            "@Builder\nexport function LogBuilder() {}\n@Builder\nexport function LogDetailsBuilder() {}\n",
        )
        .expect("write");
        let routes = route_entries(
            Some(&module_with(vec![RouterMapDef {
                name: "LogDetails".to_string(),
                page: Some("components/More/Log.ets".to_string()),
                build_function: None,
            }])),
            &dir,
        )
        .expect("resolves");
        assert_eq!(routes[0].build_function, "LogDetailsBuilder");
        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    /// A builder that lives in another file is refused, even though it exists.
    #[test]
    fn a_builder_from_another_file_is_refused() {
        let dir = std::env::temp_dir().join(format!("hvolt-routes5-{}", std::process::id()));
        let page = dir.join("components/Log.ets");
        let elsewhere = dir.join("components/Elsewhere.ets");
        std::fs::create_dir_all(page.parent().unwrap()).expect("mkdir");
        std::fs::write(&page, "@Builder\nexport function LogBuilder() {}\n").expect("write");
        std::fs::write(
            &elsewhere,
            "@Builder\nexport function ElsewhereBuilder() {}\n",
        )
        .expect("write");
        let routes = route_entries(
            Some(&module_with(vec![RouterMapDef {
                name: "Elsewhere".to_string(),
                page: Some("components/Log.ets".to_string()),
                build_function: Some("ElsewhereBuilder".to_string()),
            }])),
            &dir,
        );
        assert!(
            routes.is_err(),
            "a builder absent from the page must not resolve"
        );
        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    /// A route that names only a file with several builders is ambiguous, not "the first one".
    ///
    /// `Common.ets` declares `BackButton`, `BindSheetTitleCustomBuilder` and
    /// `RightDownButton`; taking the first registered `Common` against `BackButton`.
    #[test]
    fn a_file_with_several_builders_and_no_name_match_is_refused() {
        let dir = std::env::temp_dir().join(format!("hvolt-routes6-{}", std::process::id()));
        let page = dir.join("components/Common/Common.ets");
        std::fs::create_dir_all(page.parent().unwrap()).expect("mkdir");
        std::fs::write(
            &page,
            "@Builder\nexport function BackButton() {}\n@Builder\nexport function RightDownButton() {}\n",
        )
        .expect("write");
        let routes = route_entries(
            Some(&module_with(vec![RouterMapDef {
                name: "Common".to_string(),
                page: None,
                build_function: None,
            }])),
            &dir,
        );
        assert!(
            routes.is_err(),
            "a file with several builders and no name match must not resolve"
        );
        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    /// The same file resolves once a builder really answers to the route name.
    #[test]
    fn a_named_builder_wins_over_the_files_builders() {
        let dir = std::env::temp_dir().join(format!("hvolt-routes7-{}", std::process::id()));
        let page = dir.join("components/Common/Common.ets");
        std::fs::create_dir_all(page.parent().unwrap()).expect("mkdir");
        std::fs::write(
            &page,
            "@Builder\nexport function BackButton() {}\n@Builder\nexport function CommonBuilder() {}\n",
        )
        .expect("write");
        let routes = route_entries(
            Some(&module_with(vec![RouterMapDef {
                name: "Common".to_string(),
                page: None,
                build_function: None,
            }])),
            &dir,
        )
        .expect("resolves");
        assert_eq!(routes[0].build_function, "CommonBuilder");
        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    /// A route whose name matches only a file stem takes that file's single builder.
    #[test]
    fn a_stem_match_needs_the_file_to_hold_one_builder() {
        let dir = std::env::temp_dir().join(format!("hvolt-routes8-{}", std::process::id()));
        let page = dir.join("components/EditArrayConfig.ets");
        std::fs::create_dir_all(page.parent().unwrap()).expect("mkdir");
        std::fs::write(
            &page,
            "@Builder\nexport function ArrayConfigEditorBuilder() {}\n",
        )
        .expect("write");
        let routes = route_entries(
            Some(&module_with(vec![RouterMapDef {
                name: "EditArrayConfig".to_string(),
                page: None,
                build_function: None,
            }])),
            &dir,
        )
        .expect("resolves");
        assert_eq!(routes[0].build_function, "ArrayConfigEditorBuilder");
        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    /// Two files with the same stem are ambiguous even when each holds one builder.
    #[test]
    fn a_stem_shared_by_two_files_is_refused() {
        let dir = std::env::temp_dir().join(format!("hvolt-routes9-{}", std::process::id()));
        for sub in ["components/Common.ets", "widgets/Common.ets"] {
            let page = dir.join(sub);
            std::fs::create_dir_all(page.parent().unwrap()).expect("mkdir");
            std::fs::write(&page, "@Builder\nexport function CommonBuilder() {}\n").expect("write");
        }
        let routes = route_entries(
            Some(&module_with(vec![RouterMapDef {
                name: "Common".to_string(),
                page: None,
                build_function: None,
            }])),
            &dir,
        );
        // Two builders answer to the name as well — either way the answer is refusal.
        assert!(routes.is_err(), "a shared stem must not resolve");
        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    /// Both decorator spellings count as a builder.
    #[test]
    fn local_builders_are_builders() {
        let names = builder_names(
            "@LocalBuilder\nfunction x() {}\n@Builder\nexport function yBuilder() {}\n",
        );
        assert_eq!(names, vec!["x", "yBuilder"]);
    }
}
