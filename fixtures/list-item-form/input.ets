// `process_component_build.js` `checkLazyCreate`: a `ListItem` outside a `LazyForEach` is built
// through the item-generation form — `ListItem.create` takes `deepRenderFunction` as its first
// argument, and `itemCreation` / `itemCreation2` / `deepRenderFunction` carry the create call,
// the attribute chain, and the children.
//
// The `ForEach` item below is inside a plain loop, not a `LazyForEach`, so it takes the same
// form; a `ListItem` that is a `LazyForEach` root keeps the `create(() => { }, false)` shape.
@Entry
@Component
struct Index {
  @State rows: string[] = ['a', 'b']

  build() {
    List({ space: 4 }) {
      ListItem() {
        Text('static')
      }
      .padding(8)
      .width('100%')

      ForEach(this.rows, (row: string) => {
        ListItem() {
          Text(row)
        }
        .height(20)
      }, (row: string) => row)
    }
    .width('100%')
  }
}
