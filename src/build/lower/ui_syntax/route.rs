//! Which lowering route a source file needs.
//!
//! The marker is the decorator itself, not the file name or directory: a `.ets` file in
//! any module may declare either protocol.

pub(crate) enum Route {
    /// No component declaration — plain transpile.
    None,
    /// `@Component` / `@ComponentV2`-free component (ViewPU).
    V1,
    /// `@ComponentV2` (ViewV2).
    V2,
}

pub(crate) fn detect(source: &str) -> Route {
    // The decorators are read as tokens: a `@Component` written inside a string or a comment
    // does not make the file a component.
    if crate::build::lex::has_decorator(source, &["ComponentV2"]) {
        return Route::V2;
    }
    // A `@CustomDialog` struct is a component too: without this it takes the plain path and
    // its field decorators (`@State`, `@Prop`) end up in the output.
    if crate::build::lex::has_decorator(source, &["Component", "CustomDialog"]) {
        return Route::V1;
    }
    // A file may declare no struct at all and still hold view code: a top-level
    // `@Builder function` builds through the same stack. Its route is the same one; the
    // lowering then decides from the table whether it actually has anything to expand and
    // falls back to the plain path when it does not (`PipManager.ets`, `Popup.ets`).
    if crate::build::lex::has_decorator(source, &["Builder", "Extend"]) {
        return Route::V1;
    }
    Route::None
}
