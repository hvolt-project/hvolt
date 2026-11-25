
@Entry
@Component
struct BatchProbe74a {
  build() {
    Column() {
      try {
        Text("x")
      } catch (e) {
        Text("y")
      }
    }
  }
}
