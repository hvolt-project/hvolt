//! V2 lowering: `@ComponentV2` struct → `ViewV2` class.
//!
//! The compiled shape is the V1 shape with two differences, both verified by disassembling
//! a DevEco-built V2 component (`xb_components...Xb_SettingItem`) and diffing its member
//! set against a V1 page:
//!
//! * the base class is `ViewV2` instead of `ViewPU`;
//! * V2 carries `resetStateVarsOnReuse`, which V1 does not.
//!
//! Everything else (`initialRender`, `updateStateVars`, `purgeVariableDependenciesOnElmtId`,
//! `setInitiallyProvidedValue`, `rerender`, `paramsLambda`) is identical, so the expansion
//! is shared and only the protocol differs.

use crate::build::lower::ets_component::expand_component_file_with;
use crate::build::lower::ets_transpile::TranspileOptions;
use crate::build::stage::utils::V2_PROTOCOL;

pub(crate) fn transform(source: &str, opts: &TranspileOptions) -> Result<String, String> {
    expand_component_file_with(source, opts, V2_PROTOCOL)
}
