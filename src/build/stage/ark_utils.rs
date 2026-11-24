// process stage: `ark_utils.js` — ohm-url and package-name derivation for module specifiers.
//
// The reference derives the record name a file is emitted under and the package an ohm-url
// belongs to. hvolt derives the same names here, and `crate::build::compile::abc_compile`
// calls these for its `filesInfo.txt` rows; the module table the reference keeps in its
// environment is passed in explicitly instead of being read from a global.

/// `getOhmUrlByFilepath`: the record name for one source file.
///
/// `ts_root` is the staged source root and `entry_module` the module the record belongs to.
/// The reference reads both from its environment; hvolt carries one environment per merge,
/// so they are arguments.
pub(crate) fn get_ohm_url_by_filepath(
    ts_root: &std::path::Path,
    ts_path: &std::path::Path,
    entry_module: &str,
) -> String {
    let rel = ts_path
        .strip_prefix(ts_root)
        .unwrap_or(ts_path)
        .to_string_lossy()
        .replace('\\', "/");
    let rel = rel.trim_end_matches(".ts");
    let rel = rel.trim_end_matches(".js");
    // A package entry (`from '<pkg>'`) is addressed as `<pkg>.Index` — no source root.
    let rel = match rel.split_once('/') {
        // A member module's package entry is staged at the module root, so its record has no
        // source root (`&proxy_core/Index&1.0.0`). The entry module has no package entry: its
        // `entryPath` is the source root, so a file of its own keeps the full form.
        //
        // A shared package is the exception among member modules: its package entry is
        // `src/main/Index` (not `Index` at the module root), so it takes the ordinary
        // `module_path` form — which is exactly `&shared.src.main.Index&`.
        Some((module, entry))
            if module != entry_module
                && !is_shared_module(module)
                && module_entry(module) == entry =>
        {
            module_entry_path(module)
        }
        Some((module, rest)) => module_path(module, rest),
        None => rel.to_string(),
    };
    let rel = rel.as_str();
    // The generated ResourceTable header lives at
    // `<module>/build/<mode>/generated/r/<mode>/ResourceTable.{js,ts}` and
    // the reference names its record `<module>/build/generated/r/ResourceTable` —
    // the build-mode segment is dropped from both occurrences.
    if let Some(i) = rel.find("/build/") {
        let tail = &rel[i + "/build/".len()..];
        if tail.starts_with("default/generated/r/default/") {
            let head = &rel[..i + "/build/".len()];
            return format!("{}generated/r/ResourceTable", head);
        }
    }
    rel.to_string()
}

/// `getNormalizedOhmUrlByAliasName`: the normalized ohm-url for one alias request.
///
/// An alias inside a package is addressed as `&<pkg>.<path>&<version>`; the caller supplies the
/// package name and the path the alias resolved to.
#[allow(dead_code)] // 记录名的包形式；import 侧接线后由它调用
pub(crate) fn get_normalized_ohm_url_by_alias_name(alias_name: &str, package_name: &str) -> String {
    let alias = alias_name.trim_start_matches('&');
    format!("&{package_name}.{alias}&")
}

/// `transformOhmurlToPkgName`: the package a record name belongs to, when it is one.
///
/// A record the project itself owns (`&<module>/...`) carries no package segment, so nothing is
/// answered for it; a third-party record answers the segment before its first dot.
#[allow(dead_code)] // 反向查询；import 侧接线后由它调用
pub(crate) fn transform_ohm_url_to_pkg_name(ohm_url: &str) -> Option<String> {
    let trimmed = ohm_url.trim_start_matches('&');
    let (head, _) = trimmed.split_once('.')?;
    if head.is_empty() {
        return None;
    }
    // 项目自己的记录以模块名开头，没有包段；作用域包名（如 @scope/name）本身含一个斜杠，
    // 它仍然是包名。
    if head.contains('/') && !head.starts_with('@') {
        return None;
    }
    Some(head.to_string())
}

/// A member module's package entry path.
pub(crate) fn module_entry(module: &str) -> &'static str {
    crate::build::compile::abc_compile::module_entry(module)
}

/// Whether a module is a shared (HSP) package.
pub(crate) fn is_shared_module(module: &str) -> bool {
    crate::build::compile::abc_compile::is_shared_module(module)
}

/// The record name of a member module's package entry.
pub(crate) fn module_entry_path(module: &str) -> String {
    crate::build::compile::abc_compile::module_entry_path(module)
}

/// `<module>.<rest>`: the record name of a module-internal file.
pub(crate) fn module_path(module: &str, rel: &str) -> String {
    crate::build::compile::abc_compile::module_path(module, rel)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An alias inside a package is addressed as `&<pkg>.<path>&`.
    #[test]
    fn normalized_url_names_the_package() {
        assert_eq!(
            get_normalized_ohm_url_by_alias_name("@mcui/mccharts", "entry"),
            "&entry.@mcui/mccharts&"
        );
        assert_eq!(
            get_normalized_ohm_url_by_alias_name("&library/src/main/ets/Index", "library"),
            "&library.library/src/main/ets/Index&"
        );
    }

    /// A project-owned record has no package segment; a package record answers it.
    #[test]
    fn package_from_record_name() {
        assert_eq!(
            transform_ohm_url_to_pkg_name("&entry/src/main/ets/pages/Index&"),
            None
        );
        assert_eq!(
            transform_ohm_url_to_pkg_name("&shared/src/main/ets/Index&"),
            None
        );
        assert_eq!(
            transform_ohm_url_to_pkg_name("&@mcui/mccharts.Index&2.8.9"),
            Some("@mcui/mccharts".to_string())
        );
    }
}
