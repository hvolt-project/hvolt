// ets_resources: resolve `$r('app.<type>.<name>')` against restool's id table.
//
// The loader replaces each `$r` call with a plain object literal carrying the resource id,
// its type code, and the owning bundle and module; `es2abc` then folds that into a
// `createobjectwithbuffer` instruction. Measured on a real page:
//
//   $r('app.string.module_desc')  ->  { "id": 16777222, "type": 10003, params: [], "bundleName", "moduleName" }
//   $r('app.media.startIcon')     ->  { "id": 16777225, "type": 20000, params: [], "bundleName", "moduleName" }
//
// The ids come from `ResourceTable.txt`, which `restool` writes next to the compiled
// resources (`<type> <name> 0x<id>` per line).
//
// `moduleName` is the module the package is built for, not the module that owns the source
// file: `addBundleAndModuleParam` in the SDK's `ets-loader/lib/process_ui_syntax.js` writes
// `createBundleOrModuleNode(false, "moduleName")`, which is `projectConfig.moduleName`, and
// hvigor sets that once per CompileArkTS task (the module being assembled). Measured on
// clashbox: `xb_components` sources that use `entry` resources record `moduleName: "entry"`.
//
// `$rawfile(arg)` is the same literal with a file name instead of a table id. The loader
// resolves a literal name at build time (`id: 0`, the name in `params`) but has nothing to
// resolve for an expression, so it keeps the expression and writes `id: -1`:
//
//   $rawfile('xiaobai.jpg')    ->  { "id": 0, "type": 30000, params: ['xiaobai.jpg'], … }
//   $rawfile(this.startImage)  ->  { "id": -1, "type": 30000, params: [this.startImage], … }
//
// Left as a call, `$rawfile` becomes a read of an undefined global at run time.

use std::collections::HashMap;

/// One entry of `ResourceTable.txt`: a resource kind, a name, and its numeric id.
pub(crate) type ResourceTable = HashMap<(String, String), u32>;

/// The `type` codes `resource.d.ts` documents for `Resource.type`.
fn type_code(kind: &str) -> Option<u32> {
    Some(match kind {
        "color" => 10001,
        "float" => 10002,
        "string" => 10003,
        "plural" => 10004,
        "boolean" => 10005,
        "intarray" => 10006,
        "integer" => 10007,
        "pattern" => 10008,
        "strarray" => 10009,
        "media" => 20000,
        "rawfile" => 30000,
        "symbol" => 40000,
        _ => return None,
    })
}

/// Read `ResourceTable.txt`. Unknown lines are skipped: the file also carries a header.
pub(crate) fn parse_table(text: &str) -> ResourceTable {
    let mut table = ResourceTable::new();
    for line in text.lines() {
        let mut parts = line.split_whitespace();
        let (Some(kind), Some(name), Some(id)) = (parts.next(), parts.next(), parts.next()) else {
            continue;
        };
        let Some(id) = id
            .strip_prefix("0x")
            .and_then(|h| u32::from_str_radix(h, 16).ok())
        else {
            continue;
        };
        table.insert((kind.to_string(), name.to_string()), id);
    }
    table
}

/// The SDK's system resource ids, loaded once from `sysResource.js` next to the ets-loader.
fn sys_resource_id(kind: &str, name: &str) -> Option<u32> {
    static TABLE: std::sync::OnceLock<std::collections::HashMap<(String, String), u32>> =
        std::sync::OnceLock::new();
    let table = TABLE.get_or_init(load_sys_resources);
    table.get(&(kind.to_string(), name.to_string())).copied()
}

/// Parse `module.exports.sys = { kind: { name: id, … }, … }`.
fn load_sys_resources() -> std::collections::HashMap<(String, String), u32> {
    let mut table = std::collections::HashMap::new();
    let Some(path) = sys_resource_path() else {
        return table;
    };
    let Ok(text) = std::fs::read_to_string(&path) else {
        return table;
    };
    let mut kind = String::new();
    let mut depth = 0usize;
    for line in text.lines() {
        let trimmed = line.trim();
        if depth <= 1 {
            if let Some((key, rest)) = trimmed.split_once('{') {
                if let Some(key) = key.trim().strip_suffix(':') {
                    let key = key.trim().trim_matches(|c| c == '"' || c == '\'');
                    if !key.is_empty() && !rest.contains(':') {
                        kind = key.to_string();
                        depth += 1;
                        continue;
                    }
                }
            }
        }
        if trimmed.contains('{') {
            depth += 1;
        }
        if let Some((name, value)) = trimmed.split_once(':') {
            let name = name
                .trim()
                .trim_matches(|c| c == '"' || c == '\'' || c == ',');
            let value = value.trim().trim_end_matches(',');
            if let Ok(id) = value.parse::<u32>() {
                if !kind.is_empty() && !name.is_empty() {
                    table.insert((kind.clone(), name.to_string()), id);
                }
            }
        }
        if trimmed.starts_with('}') && depth > 0 {
            depth -= 1;
        }
    }
    table
}

/// Locate `sysResource.js`: `$HVOLT_SYS_RESOURCE`, else the first SDK under `~/.hvolt/sdk`.
fn sys_resource_path() -> Option<std::path::PathBuf> {
    if let Ok(path) = std::env::var("HVOLT_SYS_RESOURCE") {
        let path = std::path::PathBuf::from(path);
        if path.is_file() {
            return Some(path);
        }
    }
    let home = std::env::var("HOME").ok()?;
    let sdk_root = std::path::PathBuf::from(home).join(".hvolt/sdk");
    let mut versions: Vec<std::path::PathBuf> = std::fs::read_dir(&sdk_root)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    versions.sort();
    for version in versions.into_iter().rev() {
        let candidate = version.join("openharmony/ets/build-tools/ets-loader/sysResource.js");
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

/// The literal the runtime expects for one resolved raw file.
fn rawfile_literal(name: &str, bundle: &str, module: &str) -> String {
    format!(
        "{{ \"id\": 0, \"type\": 30000, params: ['{name}'], \"bundleName\": '{bundle}', \"moduleName\": '{module}' }}"
    )
}

/// The literal the runtime expects for a raw file named by an expression.
///
/// The loader cannot resolve the name against the id table, so it writes `id: -1` and copies
/// every argument as written into `params` (`process_ui_syntax.js::createResourceParamWithVariable`,
/// reached from `processResourceData` for a non-literal `$rawfile`). Measured on clashbox:
///   `$rawfile(rawfileName)`     -> `{ "id": -1, "type": 30000, params: [rawfileName], … }`
///   `$rawfile(this.startImage)` -> `{ "id": -1, "type": 30000, params: [this.startImage], … }`
fn rawfile_value_literal(params: &[String], bundle: &str, module: &str) -> String {
    format!(
        "{{ \"id\": -1, \"type\": 30000, params: [{}], \"bundleName\": \"{bundle}\", \"moduleName\": \"{module}\" }}",
        params.join(", ")
    )
}

/// The literal the runtime expects for one resolved resource.
///
/// `$r('app.string.x')` carries no argument and its `params` list stays empty; a call with
/// arguments — `$r('app.string.copy_to_blacklist', item.name)` — carries them into `params` as
/// written (measured on clashbox: `{ "id": 16777501, "type": 10003, params: [item.name], … }`).
fn literal(id: u32, code: u32, params: &[String], bundle: &str, module: &str) -> String {
    let params = if params.is_empty() {
        "[]".to_string()
    } else {
        format!("[{}]", params.join(", "))
    };
    format!(
        "{{ \"id\": {id}, \"type\": {code}, params: {params}, \"bundleName\": '{bundle}', \"moduleName\": '{module}' }}"
    )
}

/// Replace every `$r('app.<kind>.<name>')` / `$rawfile(…)` with the object literal the runtime
/// expects.
///
/// A reference whose resource is missing from the table is left untouched, so the caller
/// can report it rather than silently emitting a bad id. A `$rawfile` call always expands: a
/// literal file name resolves at build time, an expression is carried to the runtime.
pub(crate) fn rewrite(
    source: &str,
    table: &ResourceTable,
    bundle: &str,
    module: &str,
) -> (String, Vec<String>) {
    // `$r('app.…')` / `$r('sys.…')` is a resource reference the runtime resolves through the
    // table. The call sites come from the tree, so `$r` inside a string or a comment is data;
    // the text between replacements is copied byte for byte.
    let Some(calls) = crate::build::ets_transform::resource_calls(source) else {
        if crate::build::ets_transform::parse_diagnostics_enabled() {
            eprintln!("resource rewrite skipped: the module does not parse");
        }
        return (source.to_string(), Vec::new());
    };
    let mut edits: Vec<(usize, usize, String)> = Vec::new();
    let mut missing = Vec::new();
    for call in calls {
        let (key, params) = match &call.kind {
            // A raw file is not in the id table: the loader writes `id: 0` with the file name in
            // `params`, and the type code 30000 (`rawfile`) tells the runtime to read it that way
            // (measured: `ContributorListOp.ts`, `icon: { "id": 0, "type": 30000, params:
            // ['xiaobai.jpg'], … }`).
            crate::build::ets_transform::ResourceKind::RawFileName { name } => {
                edits.push((call.start, call.end, rawfile_literal(name, bundle, module)));
                continue;
            }
            // A raw file named by an expression has no id either: the loader cannot look it up
            // and keeps the expression for the runtime, with `id: -1`.
            crate::build::ets_transform::ResourceKind::RawFileValue { params } => {
                edits.push((
                    call.start,
                    call.end,
                    rawfile_value_literal(params, bundle, module),
                ));
                continue;
            }
            crate::build::ets_transform::ResourceKind::Named { key, params } => (key, params),
        };
        // System resources resolve through the SDK's own table (`sysResource.js`), which is
        // what the official loader reads; leaving `$r('sys.…')` in the output makes the id
        // come out as 0 at runtime.
        if let Some(rest) = key.strip_prefix("sys.") {
            let resolved = rest.split_once('.').and_then(|(kind, name)| {
                Some((kind, name, sys_resource_id(kind, name)?, type_code(kind)?))
            });
            match resolved {
                Some((_, _, id, code)) => edits.push((
                    call.start,
                    call.end,
                    literal(id, code, params, bundle, module),
                )),
                None => missing.push(key.to_string()),
            }
            continue;
        }
        let Some(rest) = key.strip_prefix("app.") else {
            continue;
        };
        let Some((kind, name)) = rest.split_once('.') else {
            continue;
        };
        let (Some(id), Some(code)) = (
            table.get(&(kind.to_string(), name.to_string())),
            type_code(kind),
        ) else {
            missing.push(key.to_string());
            continue;
        };
        edits.push((
            call.start,
            call.end,
            literal(*id, code, params, bundle, module),
        ));
    }
    (
        crate::build::ets_assemble::apply_replacements(source, &edits),
        missing,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_system_resource_resolves_to_the_sdk_id() {
        // `sys.color.icon_emphasize` is 125830995 in the SDK's `sysResource.js`; leaving the
        // `$r(…)` call in place makes the runtime report "Invalid resource ID: 0".
        //
        // The id lives in the SDK, which a clean checkout and CI do not have: with no table the
        // lookup has nothing to resolve against and the reference is reported missing. Skip
        // there — set `HVOLT_SYS_RESOURCE` to the SDK's `sysResource.js` to run it.
        if sys_resource_id("color", "icon_emphasize").is_none() {
            eprintln!("skipped: no sysResource.js; set HVOLT_SYS_RESOURCE to run this test");
            return;
        }
        let table = parse_table(TABLE);
        let (out, missing) = rewrite(
            "ThemeControl.setDefaultTheme($r('sys.color.icon_emphasize'))",
            &table,
            "com.x",
            "entry",
        );
        assert!(missing.is_empty(), "{missing:?}");
        assert!(
            out.contains(r#"{ "id": 125830995, "type": 10001, params: []"#),
            "{out}"
        );
    }

    const TABLE: &str = "\
string module_desc 0x01000006
media startIcon 0x01000009
color start_window_background 0x01000007
";

    #[test]
    fn the_table_parses_hex_ids() {
        let table = parse_table(TABLE);
        assert_eq!(
            table.get(&("string".into(), "module_desc".into())).copied(),
            Some(16777222)
        );
        assert_eq!(
            table.get(&("media".into(), "startIcon".into())).copied(),
            Some(16777225)
        );
    }

    #[test]
    fn a_string_reference_becomes_an_id_and_type() {
        let table = parse_table(TABLE);
        let (out, missing) = rewrite(
            "Text($r('app.string.module_desc'))",
            &table,
            "com.x",
            "entry",
        );
        assert!(missing.is_empty(), "{missing:?}");
        assert!(
            out.contains(r#"{ "id": 16777222, "type": 10003, params: [], "bundleName": 'com.x', "moduleName": 'entry' }"#),
            "{out}"
        );
    }

    #[test]
    fn a_media_reference_uses_the_media_code() {
        let table = parse_table(TABLE);
        let (out, _) = rewrite("Image($r('app.media.startIcon'))", &table, "com.x", "entry");
        assert!(out.contains(r#""type": 20000"#), "{out}");
    }

    #[test]
    fn a_missing_resource_is_reported_and_left_alone() {
        let table = parse_table(TABLE);
        let (out, missing) = rewrite("Text($r('app.string.nope'))", &table, "com.x", "entry");
        assert_eq!(missing, vec!["app.string.nope".to_string()]);
        assert!(out.contains("$r('app.string.nope')"), "{out}");
    }

    #[test]
    fn double_quoted_references_also_resolve() {
        let table = parse_table(TABLE);
        let (out, _) = rewrite(
            "Text($r(\"app.color.start_window_background\"))",
            &table,
            "com.x",
            "entry",
        );
        assert!(out.contains(r#""type": 10001"#), "{out}");
    }

    /// `$rawfile(rawfileName)`: the loader cannot resolve a name it only learns at run time, so
    /// it writes `id: -1` and copies the argument into `params` (measured on clashbox
    /// `CodeEditor.ts`; the call would otherwise reach the runtime as an undefined global).
    #[test]
    fn a_raw_file_named_by_an_expression_keeps_the_expression() {
        let table = parse_table(TABLE);
        let (out, missing) = rewrite(
            "response.setResponseData($rawfile(rawfileName));",
            &table,
            "org.xbgroup.clashboxLTS",
            "entry",
        );
        assert!(missing.is_empty(), "{missing:?}");
        assert_eq!(
            out,
            "response.setResponseData({ \"id\": -1, \"type\": 30000, params: [rawfileName], \"bundleName\": \"org.xbgroup.clashboxLTS\", \"moduleName\": \"entry\" });"
        );
    }

    /// `$rawfile(this.startImage)`: the argument is kept as written, member access included
    /// (measured on clashbox `Xb_GeneralListItem.ts`, which builds an `Image` from it).
    #[test]
    fn a_raw_file_member_expression_keeps_its_owner() {
        let table = parse_table(TABLE);
        let (out, missing) = rewrite(
            "Image($rawfile(this.startImage))",
            &table,
            "org.xbgroup.clashboxLTS",
            "entry",
        );
        assert!(missing.is_empty(), "{missing:?}");
        assert_eq!(
            out,
            "Image({ \"id\": -1, \"type\": 30000, params: [this.startImage], \"bundleName\": \"org.xbgroup.clashboxLTS\", \"moduleName\": \"entry\" })"
        );
    }

    /// A literal file name is resolved at build time, so its literal keeps `id: 0` and the
    /// name as the one `params` entry (`ContributorListOp.ts`).
    #[test]
    fn a_literal_raw_file_name_still_uses_id_zero() {
        let table = parse_table(TABLE);
        let (out, _) = rewrite("Image($rawfile('xiaobai.jpg'))", &table, "com.x", "entry");
        assert_eq!(
            out,
            "Image({ \"id\": 0, \"type\": 30000, params: ['xiaobai.jpg'], \"bundleName\": 'com.x', \"moduleName\": 'entry' })"
        );
    }

    /// The pass is located on the tree: the same text inside a comment is data.
    #[test]
    fn a_raw_file_in_a_comment_is_left_alone() {
        let table = parse_table(TABLE);
        let (out, _) = rewrite(
            "// Image($rawfile(this.startImage))\nImage($rawfile(this.startImage))",
            &table,
            "com.x",
            "entry",
        );
        assert!(
            out.starts_with("// Image($rawfile(this.startImage))\nImage({"),
            "{out}"
        );
        assert_eq!(out.matches("$rawfile").count(), 1, "{out}");
    }
}
