// process stage: `ark_utils.js` — ohm-url and package-name derivation for module specifiers.
//
// The reference turns a source path into the record name the runtime links against
// (`getOhmUrlByFilepath`, `getNormalizedOhmUrlByFilepath`), maps a package name to the
// normalized ohm-url (`getNormalizedOhmUrlByAliasName`), and decides from the transformed
// module specifier whether a request is a system API or a package
// (`getOhmUrlBySystemApiOrLibRequest`, `transformOhmurlToPkgName`).
//
// hvolt writes the same record names, but derives them in
// `crate::build::compile::abc_compile` together with the es2abc merge, so this stage has no
// separate implementation yet.
#![allow(dead_code)] // the record names are derived in build/compile/abc_compile.rs today

/// `getOhmUrlByFilepath`: the record name for one source file.
#[allow(clippy::todo)] // the stage is not implemented; the signature is the reference's
pub(crate) fn get_ohm_url_by_filepath(_file_path: &std::path::Path) -> String {
    todo!()
}

/// `getNormalizedOhmUrlByAliasName`: the normalized ohm-url for one alias request.
#[allow(clippy::todo)] // the stage is not implemented; the signature is the reference's
pub(crate) fn get_normalized_ohm_url_by_alias_name(
    _alias_name: &str,
    _package_name: &str,
) -> String {
    todo!()
}

/// `transformOhmurlToPkgName`: the package a record name belongs to, when it is one.
#[allow(clippy::todo)] // the stage is not implemented; the signature is the reference's
pub(crate) fn transform_ohm_url_to_pkg_name(_ohm_url: &str) -> Option<String> {
    todo!()
}
