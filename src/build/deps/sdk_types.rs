// The SDK declaration table stays available for a more exact judgement than the
// current positional one; nothing reads it yet, so the whole module is dormant.
#![allow(dead_code)]

// sdk_types: does a system module declare a symbol as a value or as a type?
//
// `import type X` is only an optimisation until it is wrong: the runtime rejects a value
// read of a type-only binding (`KeyCode is not defined`).  Enums are the precise trap: their
// members look like type members (`{ key: KeyCode.KEYCODE_A }`) while they are values.
// Everything else keeps the usage-shape heuristic, which matches the reference build.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DeclKind {
    Value,
    /// A declaration that is only a type. Nothing classifies a symbol as one yet; the variant
    /// exists so the cache can record it once the resolution pass distinguishes the two.
    #[allow(dead_code)]
    Type,
}

type Cache = Mutex<HashMap<String, Option<HashMap<String, DeclKind>>>>;

static CACHE: std::sync::OnceLock<Cache> = std::sync::OnceLock::new();

fn cache() -> &'static Cache {
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// The `ets/api` directory of the resolved SDK, if it exists.
fn api_dir() -> Option<PathBuf> {
    let root = crate::sdk::manage::sdk_root()?;
    let base = root
        .join("default")
        .join("openharmony")
        .join("ets")
        .join("api");
    if base.is_dir() {
        Some(base)
    } else {
        None
    }
}

/// Every symbol one module declares, with its kind.
fn module_symbols(module: &str) -> Option<HashMap<String, DeclKind>> {
    if let Some(hit) = cache().lock().ok()?.get(module) {
        return hit.clone();
    }
    let computed = api_dir().and_then(|dir| {
        let path = dir.join(format!("@ohos.{module}.d.ts"));
        let text = std::fs::read_to_string(&path).ok()?;
        let mut map = HashMap::new();
        for line in text.lines() {
            let t = line.trim();
            for (keyword, kind) in [
                ("declare enum ", DeclKind::Value),
                ("export declare enum ", DeclKind::Value),
                ("export enum ", DeclKind::Value),
            ] {
                if let Some(rest) = t.strip_prefix(keyword) {
                    let name: String = rest
                        .chars()
                        .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '$')
                        .collect();
                    if !name.is_empty() {
                        map.entry(name).or_insert(kind);
                    }
                }
            }
        }
        if map.is_empty() {
            None
        } else {
            Some(map)
        }
    });
    cache()
        .lock()
        .ok()?
        .insert(module.to_string(), computed.clone());
    computed
}

/// The kind `module` (e.g. `multimodalInput.keyCode`) gives `symbol`, when the SDK says.
pub fn declared_kind(module: &str, symbol: &str) -> Option<DeclKind> {
    module_symbols(module)?.get(symbol).copied()
}
