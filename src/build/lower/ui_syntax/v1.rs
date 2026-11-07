//! V1 lowering: `@Component` struct → `ViewPU` class.
//!
//! The implementation still lives in [`crate::build::lower::ets_component`]; this module is the
//! named route so that callers do not depend on that file directly.

use crate::build::lower::ets_transpile::TranspileOptions;

pub(crate) fn transform(source: &str, opts: &TranspileOptions) -> Result<String, String> {
    crate::build::lower::ets_component::expand_component_file(source, opts)
}
