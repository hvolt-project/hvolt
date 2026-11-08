// `CUSTOM_BUILDER_PROPERTIES` (component_map.js) decides which attribute argument is a builder
// reference rather than a value: `dragPreview` and `accessibilityVirtualNode` are in the set, so
// their argument takes the `{ builder: () => { … } }` form.
//
// `CUSTOM_BUILDER_PROPERTIES_WITHOUTKEY` names the two entries whose argument is the bare arrow
// instead — `showUnit` and `create` — so `TextInput.showUnit` reaches the runtime as
// `() => { this.Unit.call(this); }`, with no `builder:` key around it.
@Entry
@Component
struct Index {
  @Builder Preview() {
    Text('preview')
  }

  @Builder VirtualNode() {
    Text('virtual')
  }

  @Builder Unit() {
    Text('unit')
  }

  build() {
    Column() {
      Text('drag')
        .dragPreview(this.Preview())
        .accessibilityVirtualNode(this.VirtualNode())

      TextInput({ placeholder: 'value' })
        .showUnit(this.Unit())
    }
  }
}
