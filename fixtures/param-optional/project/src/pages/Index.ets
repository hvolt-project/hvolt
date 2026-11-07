// `fields_with` (compile/component_index.rs) collects the decorated fields that feed a child's
// `_Params` interface and update object.  An optional member writes `name?: T`, so the `?` sits
// between the name and the `:` — a scanner that requires the identifier to be followed directly
// by `:` drops every optional `@Prop`, which is what `icon1?: Resource` and `isBackPress?`
// below would catch.
@Entry
@Component
struct Index {
  @State show: boolean = true;

  build() {
    Column() {
      if (this.show) {
        Child({ name: 'hello' })
      }
    }
  }
}

@Component
struct Child {
  @Prop icon1?: Resource;
  @Prop isBackPress?: boolean;
  @Prop name: string = '';

  build() {
    Text(this.name)
  }
}