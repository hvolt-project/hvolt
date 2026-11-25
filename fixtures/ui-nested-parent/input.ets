@Entry
@Component
struct NestedParent {
  build() {
    Column() {
      Span("outside")
    }
  }
}
