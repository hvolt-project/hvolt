
@Entry
@Component
struct TwoBuildProbe {
  build() {
    Column() {
      Text("a")
    }
  }

  build() {
    Column() {
      Text("b")
    }
  }
}
