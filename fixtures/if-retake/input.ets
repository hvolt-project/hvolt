// `ifRetakeId` (process_component_build.js) wraps the statements of an `if`/`else` branch whose
// direct child carries a literal `.id('…')`.  Only the first level qualifies: a nested `.id()`
// inside a deeper block stays unguarded.
@Entry
@Component
struct Index {
  @State proxyEnabled: boolean = false;

  build() {
    Column() {
      if (!this.proxyEnabled) {
        Row()
          .id('chart_0')
          .width('92%')
      } else {
        Row()
          .id('chart_1')
          .width('8%')
      }

      if (this.proxyEnabled) {
        Column() {
          Row()
            .id('nested_id')
        }
      }
    }
  }
}