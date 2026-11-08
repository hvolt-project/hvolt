//! ArkTS UI syntax lowering: one route per generation of the component protocol.
//!
//! The transform that turns a `.ets` source file into what `es2abc` accepts is not one
//! algorithm — it depends on which component protocol the source is written against.
//! Each generation therefore gets its own module, and this file is the only place that
//! decides which one runs:
//!
//! | route | protocol | source marker   | module |
//! |-------|----------|-----------------|--------|
//! | V1    | `ViewPU` | `@Component`    | [`v1`] |
//! | V2    | `ViewV2` | `@ComponentV2`  | [`v2`] |
//! | plain | —        | neither         | handled by the caller |
//!
//! Adding a generation (or replacing the whole lowering with an IR backend) means adding
//! a module here and a branch in [`route::detect`]; callers never change.

pub(crate) mod route;
pub(crate) mod v1;
pub(crate) mod v2;

use crate::build::lower::ets_transpile::TranspileOptions;

/// Lower one source file according to the component protocol it declares.
///
/// Returns `None` for files that declare no component (the caller keeps its plain path).
pub fn transform(source: &str, opts: &TranspileOptions) -> Option<Result<String, String>> {
    match route::detect(source) {
        route::Route::None => None,
        route::Route::V1 => Some(v1::transform(source, opts)),
        route::Route::V2 => Some(v2::transform(source, opts)),
    }
}
