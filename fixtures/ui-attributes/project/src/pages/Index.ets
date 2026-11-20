@Entry
@Component
struct UiAttributes {
  build() {
    Column() {
      Text("x").notARealAttribute(1)
    }
    .alsoNotAReal(2)
  }
}
