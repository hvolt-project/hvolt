//! Transitional façade over [`crate::build::ets`].
//!
//! Every item here forwards to the module that owns the behaviour; none of it holds logic. It
//! exists so the call sites can be moved one module at a time, and it is deleted once the last
//! of them reads from `ets` directly.

use std::collections::HashMap;

pub(crate) use crate::build::ets::build_body::UiItem;
pub(crate) use crate::build::ets::{Builder, ExtendFn, StyleFn};

/// The lowering context lives with the language layer; these names stay so the call sites can
/// be moved one module at a time.
pub(crate) use crate::build::ets::context::{
    column_shift, is_reusable_component, is_v1_field, set_column_shifts, set_compile_har,
    set_reusable_component, set_source_page, set_v1_fields, source_page,
};

pub(crate) fn mask_sendable(source: &str) -> (String, Vec<String>, Vec<String>) {
    crate::build::ets::sendable::mask_sendable(source)
}

pub(crate) fn inject_sendable(source: &str, classes: &[String], functions: &[String]) -> String {
    crate::build::ets::sendable::inject_sendable(source, classes, functions)
}

pub(crate) fn rewrite_api_available(source: &str) -> String {
    crate::build::ets::api_available::rewrite(source)
}

pub(crate) fn preprocess(source: &str) -> String {
    crate::build::ets::preprocess::preprocess(source)
}

pub(crate) fn mask_builder_decorators(
    source: &str,
) -> (String, Vec<String>, Vec<ExtendFn>, Vec<StyleFn>) {
    let masked = crate::build::ets::mask::mask_builder_decorators(source);
    (
        masked.source,
        masked.builders,
        masked.extends,
        masked.styles,
    )
}

pub(crate) fn mask_builder_decorators_with_locals(
    source: &str,
) -> (
    String,
    Vec<String>,
    Vec<ExtendFn>,
    Vec<StyleFn>,
    Vec<String>,
) {
    let masked = crate::build::ets::mask::mask_builder_decorators_with_locals(source);
    (
        masked.source,
        masked.builders,
        masked.extends,
        masked.styles,
        masked.locals,
    )
}

pub(crate) fn rewrite_builders(source: &str, builder_names: &[String]) -> String {
    crate::build::ets::children::rewrite_builders(source, builder_names)
}

pub(crate) fn rewrite_builders_with_locals(
    source: &str,
    builder_names: &[String],
    locals: &[String],
) -> String {
    crate::build::ets::children::rewrite_builders_with_locals(source, builder_names, locals)
}

pub(crate) fn strip_comments(text: &str) -> String {
    crate::build::lex::without_comments(text)
}

pub(crate) fn escape_string_literal(name: &str) -> String {
    crate::build::ets::types::escape_string_literal(name)
}

pub(crate) fn builtin_component_names() -> &'static [String] {
    crate::build::ets::types::builtin_component_names()
}

pub(crate) fn is_simple_type(type_text: Option<&str>) -> bool {
    crate::build::ets::types::is_simple_type(type_text)
}

pub(crate) fn observed_class(decorator: &str, is_simple: bool) -> &'static str {
    crate::build::ets::types::observed_class(decorator, is_simple)
}

pub(crate) fn parse_build_at(
    body: &str,
    array_types: &HashMap<String, String>,
    start_line: usize,
) -> Result<Vec<UiItem>, String> {
    crate::build::ets::build_body::parse(body, array_types, start_line)
}

pub(crate) fn emit_ui(
    items: &[UiItem],
    out: &mut String,
    indent: &str,
    builders: &[Builder],
    extends: &[ExtendFn],
    styles: &[StyleFn],
) {
    crate::build::ets::build_body::emit(items, out, indent, builders, extends, styles)
}
