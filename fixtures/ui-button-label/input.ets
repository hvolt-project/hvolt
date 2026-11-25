@Entry
@Component
struct ButtonLabel {
  build() {
    Column() {
      Button("label") {
        Text("child")
      }
    }
  }
}
