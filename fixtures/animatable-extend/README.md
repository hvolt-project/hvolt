# animatable-extend

`processAnimatableExtend` (`process_ui_syntax.js`) keeps the function name and appends
`elmtId`, `isInitialRender` and `parent`. The first-render branch registers the property with
`<Comp>.createAnimatableProperty("name", …params, (…params) => { … })` and then repeats the
attribute statements; the update branch calls `<Comp>.updateAnimatableProperty("name", …params)`.

Only the declaration is exercised here: the call site (`.animWidth(…)` becomes a bare
`animWidth(…, elmtId, isInitialRender, this)`) is emitted by `emit_attribute_list` in
`process_component_build.rs`, outside this stage's write scope; `_animatable-extend-call-site`
holds the reference golden for that rule until the shared file changes.
