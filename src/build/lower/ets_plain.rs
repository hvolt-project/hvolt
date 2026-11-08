//! The plain lowering: a file that declares no component.
//!
//! A plain `.ets`/`.ts` file still carries ArkUI-only syntax — `@Builder` functions whose bodies
//! hold component child blocks, `@Styles`/`@Extend` chains, `struct` declarations — so it is
//! masked, preprocessed and parsed exactly like a component file. What differs is only what the
//! transform does with the result: nothing is generated, every statement is carried over and
//! each `import` is rewritten from its node.
//!
//! This module is the plain counterpart of [`crate::build::lower::ets_component`]; both end in
//! [`crate::build::lower::ets_output`], which is the only place the output is laid out.

use std::collections::BTreeMap;

use crate::build::stage::process_import::{emit_import, KitEntry};

/// Lower one plain module to the TypeScript that `es2abc` compiles.
///
/// The file is emitted statement by statement in source order. An import is rewritten from its
/// node — the request it names, and the bindings the file actually reads — instead of being
/// rewritten as text after printing; every other statement is carried over unchanged.
///
/// A source the parser rejects is an error rather than a fallback: a second, text-based path
/// would produce a file the compiler cannot fully describe, and the failure would surface as a
/// missing symbol at run time instead of here.
pub(crate) fn lower(
    src: &str,
    kits: &BTreeMap<String, BTreeMap<String, KitEntry>>,
) -> Result<String, String> {
    // Keep the file as written: `settle_system_imports` later needs the original to see what
    // the lowering erased (a decorator's import, for one).
    let written = src;
    // A file that declares a top-level `@Builder function` carries view code the plain printing
    // would copy out untouched: the body is a component block, and the reference expands it into
    // the view stack whether or not the file also declares a struct (`PipManager.ets`,
    // `Popup.ets`, `Xb_Menu.ets`). The marker is the declaration itself, never the file name.
    if let Some(expanded) = crate::build::lower::ets_component::expand_helpers_only(src, kits)? {
        return Ok(expanded);
    }
    let src = &crate::build::ets_compile::rewrite_api_available(src);
    // `@Sendable` has no TypeScript spelling: blank it before the parse, put it back as the
    // `"use sendable"` directive afterwards.
    let (src, sendable_classes, sendable_functions) = crate::build::ets_compile::mask_sendable(src);
    let masked_file = crate::build::ets::mask::mask_builder_decorators(&src);
    let masked = masked_file.source;
    let mut names = masked_file.builders;
    for name in crate::build::ets::children::imported_names(&masked) {
        if !names.contains(&name) {
            names.push(name);
        }
    }
    for name in crate::build::ets_compile::builtin_component_names() {
        if !names.contains(name) {
            names.push(name.clone());
        }
    }
    let prepared = crate::build::ets_compile::rewrite_builders(
        &crate::build::ets_compile::preprocess(&masked),
        &names,
    );
    let allocator = oxc_allocator::Allocator::default();
    let ret = oxc_parser::Parser::new(&allocator, &prepared, oxc_span::SourceType::ts()).parse();
    if ret.panicked || !ret.errors.is_empty() {
        let first = ret
            .errors
            .first()
            .map(|e| format!("{e:?}"))
            .unwrap_or_else(|| "parser panicked".to_string());
        return Err(format!("plain source does not parse: {first}"));
    }
    let mut items: Vec<String> = Vec::new();
    let value_used = crate::build::stage::process_import::value_used_names(&ret.program);
    for directive in &ret.program.directives {
        items.push(directive.span.source_text(&prepared).to_string());
    }
    for statement in &ret.program.body {
        let span = oxc_span::GetSpan::span(statement);
        // A decorated class starts at its first decorator, not where the statement does: oxc
        // anchors `export class` at the keyword, so slicing from the statement span drops
        // `@Observed`/`@ObservedV2` (and the doc comment the decorator sits under). The
        // component route reads its classes the same way (`class_decorator_start`).
        let start = match statement {
            oxc_ast::ast::Statement::ClassDeclaration(class) => {
                crate::build::stage::utils::class_decorator_start(class, span.start as usize)
            }
            // `export class X` parses as an export wrapper around the declaration, and the
            // wrapper's span starts at `export`; the decorator sits on the class inside it.
            oxc_ast::ast::Statement::ExportNamedDeclaration(export) => match &export.declaration {
                Some(oxc_ast::ast::Declaration::ClassDeclaration(class)) => {
                    crate::build::stage::utils::class_decorator_start(class, span.start as usize)
                }
                _ => span.start as usize,
            },
            _ => span.start as usize,
        };
        // A re-export (`export { X } from '…'`) is printed bare: the reference drops a comment
        // written above it, while a `function` / `const` / `class` keeps its own
        // (`fixtures/barrel-comment`).
        let re_export = matches!(
            statement,
            oxc_ast::ast::Statement::ExportNamedDeclaration(export)
                if export.declaration.is_none() && export.source.is_some()
        );
        let text = if re_export {
            span.source_text(&prepared).to_string()
        } else {
            // A statement that carries a comment keeps its own spelling, so it also keeps the
            // source's indent unit — move it to the printer's four spaces, the way the
            // component route does (`fixtures/indent-unit`, `PLAIN_BLOCK`).
            crate::build::stage::utils::reindent_to_printer(
                crate::build::lower::ets_output::with_leading_comments(
                    &prepared,
                    start,
                    span.end as usize,
                ),
            )
        };
        if matches!(statement, oxc_ast::ast::Statement::ImportDeclaration(_)) {
            items.extend(emit_import(&text, &value_used, kits));
        } else {
            items.push(text);
        }
    }
    let lowered = crate::build::lower::ets_output::print_statements(&items);
    let lowered = crate::build::ets_compile::inject_sendable(
        &lowered,
        &sendable_classes,
        &sendable_functions,
    );
    // The `@kit.*` lines this pass expanded are settled here, where the file's own readings
    // are known: an unread symbol is erased and a type-only one stays `import type`, so
    // neither reaches the module's request array. Module-relative requests are left as
    // written for the assembly pass, which owns their urls.
    // `ESObject` is ArkTS's dynamic type and is printed `any` by the reference compiler
    // (`process_ui_syntax.js`): a source-shape fix, since es2abc erases type annotations
    // (measured: the rewrite does not move the bytecode of `EntryAbility`).
    Ok(crate::build::ets_transform::es_object_to_any(
        &crate::build::lower::enum_init::fill_enum_initializers(
            &crate::build::ets_transform::settle_system_imports(&lowered, written),
        ),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kit_map() -> BTreeMap<String, BTreeMap<String, KitEntry>> {
        let mut ability = BTreeMap::new();
        ability.insert(
            "UIAbility".to_string(),
            (
                "@ohos:app.ability.UIAbility".to_string(),
                "default".to_string(),
            ),
        );
        let mut kits = BTreeMap::new();
        kits.insert("@kit.AbilityKit".to_string(), ability);
        kits
    }

    /// A kit import is rewritten to its request; a binding the file only names in a type
    /// position is dropped instead, and the rest of the file is carried over.
    #[test]
    fn a_kit_import_becomes_its_request() {
        let src = "import { UIAbility } from '@kit.AbilityKit';\nclass A extends UIAbility {}\nconst x = 1;\n";
        let out = lower(src, &kit_map()).expect("lower");
        assert!(out.contains("@ohos:app.ability.UIAbility"), "{out}");
        assert!(out.contains("const x = 1;"), "{out}");
        assert!(!out.contains("@kit."), "{out}");
    }

    /// A `.so` import and an ordinary statement pass through unchanged.
    #[test]
    fn a_so_import_and_a_plain_statement_pass_through() {
        let src = "import { feed } from 'libasr.so';\nlet y = 2;\n";
        let out = lower(src, &kit_map()).expect("lower");
        assert!(out.contains("libasr.so"), "{out}");
        assert!(out.contains("let y = 2;"), "{out}");
    }

    /// A decorated exported class keeps its decorator and its doc comment: the statement span
    /// starts at `export`, so the slice has to start at the decorator
    /// (`entry/common/datasources/ConnectData.ets` in clashbox).
    #[test]
    fn a_decorated_exported_class_keeps_its_decorator() {
        let src = "import { Base } from './Base';\n\n/**\n * 连接数据源\n */\n@Observed\nexport class Items extends Base {\n  private xs: string[] = []\n}\n";
        let out = lower(src, &kit_map()).expect("lower");
        assert!(out.contains("@Observed"), "{out}");
        assert!(out.contains("连接数据源"), "{out}");
        assert!(out.contains("export class Items"), "{out}");
    }

    /// The same for `@ObservedV2`, and an undecorated class is untouched.
    #[test]
    fn an_undecorated_class_is_untouched() {
        let src = "@ObservedV2\nexport class A {}\nexport class B {}\n";
        let out = lower(src, &kit_map()).expect("lower");
        assert!(out.contains("@ObservedV2"), "{out}");
        assert!(out.contains("export class B"), "{out}");
        assert_eq!(out.matches("class B").count(), 1, "{out}");
    }

    /// A source the parser cannot accept is reported, not silently degraded to a text rewrite.
    #[test]
    fn an_unparsable_source_is_an_error() {
        let err = lower("const x = ;\n", &kit_map()).expect_err("refused");
        assert!(err.contains("does not parse"), "{err}");
    }
}
