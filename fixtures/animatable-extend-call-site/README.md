# animatable-extend-call-site

The full `@AnimatableExtend` path, including the call-site rewrite: the reference emits
`animWidth(this.target, elmtId, isInitialRender, this);` inside the component's
`observeComponentCreation2` arrow, next to the other attribute writes, and never
`Text.animWidth(…)`.

The declaration half is `processAnimatableExtend` (implemented in `process_ui_syntax.rs`); the
call-site half is `isExtendFunctionNode`/`bindComponentAttr` (`process_component_build.js`),
implemented in `process_component_build.rs::emit_attribute_list`.
