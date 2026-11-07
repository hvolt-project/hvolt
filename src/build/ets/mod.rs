//! The ArkTS language layer: everything the lowering needs to turn ArkTS source into the
//! TypeScript that `es2abc` accepts.
//!
//! Each module answers one question, and none of them emits a file layout — that belongs to
//! [`crate::build::lower::ets_output`].
//!
//! | module | question |
//! |--------|----------|
//! | [`types`]      | is this type a value type, which observation class does a decorator pick |
//! | [`preprocess`] | how does ArkTS spelling become TypeScript spelling |
//! | [`mask`]       | which declarations cannot reach a parser, and what do they mean |
//! | [`children`]   | which call takes a child block, and how is it passed |
//! | [`build_body`] | what does a `build()` body render |
//! | [`builders`]   | what does a `@Builder` / `@Extend` / `@Styles` declaration compile to |
//! | [`sendable`]   | where does a `@Sendable` directive go |
//! | [`api_available`] | how does an API-level check become a comparison |

pub(crate) mod api_available;
// The rules moved to the stage tree; these names stay because `ets_compile` and
// `build/lower/ets_component` (both outside this refactor's write scope) address them here.
pub(crate) use crate::build::stage::process_component_build as build_body;
pub(crate) mod children;
pub(crate) mod comments;
pub(crate) mod context;
pub(crate) use crate::build::stage::process_ui_syntax as mask;
pub(crate) mod preprocess;
pub(crate) mod sendable;
pub(crate) mod types;

/// A `@Extend(Comp) function name(params) { .attr(...) }` declaration.
#[derive(Clone)]
pub(crate) struct ExtendFn {
    pub name: String,
    pub comp: String,
    /// The parameter list exactly as written (`size: number`): the emitted helper keeps the
    /// declared types.
    pub params_text: String,
    /// `[("fontSize", ["12"]), …]`: the attribute chain the body declares.
    pub body: Vec<(String, Vec<String>)>,
    /// Where the declaration stood, **in the coordinates of the text the parser reads**.
    ///
    /// The piece list that emits a helper puts it back at this offset, so the offset has to be
    /// the parser's: [`crate::build::stage::process_ui_syntax::mask`] reads the source, and the
    /// child-block rewrite widens that text before a parser sees it
    /// (`ets_component::read_component_file` translates this field with the widening it already
    /// measured).
    pub order: usize,
    /// Where the declaration stood in the source, for anything that slices the source.
    ///
    /// The comment written above the declaration is read from the source at this offset
    /// (`ets_component::with_extend_comment`): asked at [`Self::order`], the scan would read the
    /// widened text's offset out of the source and find nothing — or a comment the widening
    /// pushed the offset past.
    pub source_order: usize,
}

/// A `@Styles function name() { .attr(...) }` declaration. It has no emitted function: every
/// call site is inlined with that site's component name.
#[derive(Clone)]
pub(crate) struct StyleFn {
    /// The struct that declares the member `@Styles`, or empty for a top-level
    /// `@Styles function` (which every component in the file can call).
    pub comp: String,
    pub name: String,
    pub body: Vec<(String, Vec<String>)>,
}

/// A `@Builder function name(params) { body }` declaration.
#[derive(Clone)]
pub(crate) struct Builder {
    pub name: String,
    pub params: Vec<String>,
    pub body: String,
    /// `export function …`: the declaration has to keep its `export`, or every importer of
    /// the name fails to link.
    pub is_export: bool,
    /// Where the declaration stood in the source, so helpers keep their order.
    pub order: usize,
    /// The comment block written above the declaration, in source order: the reference prints
    /// it directly over the emitted `function` line.
    pub comment: Vec<String>,
}
