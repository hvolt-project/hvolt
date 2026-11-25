@Entry
@Component
struct AttrProbe {
  build() {
    Column() {
      Text("x")
        .notARealAttribute(1)
    }
  }
}
