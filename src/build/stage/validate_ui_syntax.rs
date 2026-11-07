// process stage: `validate_ui_syntax.js` — the declaration collection and its diagnostics.
//
// The reference walks the program once and fills `componentCollection` (custom components,
// custom dialogs, observed classes), `linkCollection` / `stateCollection` (a struct's
// decorated fields), `builderParamObjectCollection` and `enumCollection`; every later stage
// reads those collections instead of re-reading the source. It reports each violation as a
// `1090xxxx` code on the same walk.
//
// hvolt has no diagnostic layer: the lowering reports a `String` and the build continues, so
// the collection entry and the diagnostic entry are both absent. Their signatures are the
// reference's — `validateUISyntax(sourceText, sourceFile, fileName, errors, moduleName)` at
// `validate_ui_syntax.js:114` — so wiring one later does not change a caller.
#![allow(dead_code)] // not wired: the lowering reports errors as strings today

use std::collections::{BTreeMap, BTreeSet};

/// `ComponentCollection`: what `validateUISyntax` records about one program.
#[derive(Default)]
pub(crate) struct ComponentCollection {
    /// `componentCollection.customComponents`.
    pub(crate) custom_components: BTreeSet<String>,
    /// `componentCollection.customDialogs`: the structs `createControllerSet` keys on.
    pub(crate) custom_dialogs: BTreeSet<String>,
    /// `observedClassCollection`.
    pub(crate) observed_classes: BTreeSet<String>,
    /// `linkCollection`: struct name → its `@Link` fields.
    pub(crate) link_collection: BTreeMap<String, BTreeSet<String>>,
    /// `stateCollection`: struct name → its `@State` fields.
    pub(crate) state_collection: BTreeMap<String, BTreeSet<String>>,
    /// `enumCollection`: every enum name, which decides `isSimpleType`.
    pub(crate) enum_collection: BTreeSet<String>,
}

/// One diagnostic the reference reports as `{type, message, pos, code}`.
pub(crate) struct TransformLog {
    pub(crate) message: String,
    pub(crate) code: String,
    pub(crate) position: usize,
}

/// `validateUISyntax(sourceText, sourceFile, fileName, errors, moduleName)`.
///
/// Fills the collection for one source file and appends a [`TransformLog`] per violation;
/// the returned collection is the one the following stages read.
#[allow(clippy::todo)] // the stage is not implemented; the signature is the reference's
pub(crate) fn validate_ui_syntax(
    _source: &str,
    _file_name: &std::path::Path,
    _errors: &mut Vec<TransformLog>,
) -> ComponentCollection {
    todo!()
}
