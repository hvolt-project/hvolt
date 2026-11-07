// stage: the compile layer split one module per ets-loader stage.
//
// The reference compiler's ETS transform is a set of stages under
// `<sdk>/ets/build-tools/ets-loader/lib/`, one file per stage. This tree mirrors that
// boundary one-to-one, so a rule can be traced from the emitted text back to the file that
// decides it:
//
// | module | ets-loader file | responsibility |
// |--------|-----------------|----------------|
// | [`pre_define`] | `pre_define.js` | every literal the transform emits, by the reference's own symbol |
// | [`utils`] | `utils.js` | the shared model (`ComponentInfo`, `FieldInfo`) and the helpers stages share |
// | [`ark_utils`] | `ark_utils.js` | ohm-url / package-name derivation for module specifiers |
// | [`process_import`] | `process_import.js` | one import declaration becomes the statements the runtime links |
// | [`process_ui_syntax`] | `process_ui_syntax.js` | the file entry: `@Extend`/`@Styles`, the assembly order, `@CustomDialog` construction |
// | [`process_component_build`] | `process_component_build.js` | a `build()` body becomes `observeComponentCreation2` statements |
// | [`process_component_class`] | `process_component_class.js` | the class header, the `_Params` interface, the member order |
// | [`process_component_member`] | `process_component_member.js` | one decorated field becomes its backstore, accessors and lifecycle lines |
// | [`process_custom_component`] | `process_custom_component.js` | a child custom component's construction, params lambda and reuse flag |
// | [`process_struct_componentV2`] | `process_struct_componentV2.js` | the `@ComponentV2` member shape |
// | [`validate_ui_syntax`] | `validate_ui_syntax.js` | the declaration collection and the diagnostic codes |
// | [`ets_checker`] | `ets_checker.js` | the ArkTS linter pass |
// | [`declaration_merger`] | `declaration_merger.js` | merged `.d.ets` declaration output |
// | [`gen_abc_plugin`] | `gen_abc_plugin.js` | the es2abc merge input |
//
// A module that hvolt does not implement yet carries the reference's entry signature,
// `todo!()` and `#[allow(dead_code)]`; the body is absent rather than guessed.

pub(crate) mod ark_utils;
pub(crate) mod declaration_merger;
pub(crate) mod ets_checker;
pub(crate) mod gen_abc_plugin;
pub(crate) mod pre_define;
pub(crate) mod process_component_build;
pub(crate) mod process_component_class;
pub(crate) mod process_component_member;
pub(crate) mod process_custom_component;
pub(crate) mod process_import;
#[allow(non_snake_case)] // the module name is the reference's file name
pub(crate) mod process_struct_componentV2;
pub(crate) mod process_ui_syntax;
pub(crate) mod utils;
pub(crate) mod validate_ui_syntax;
