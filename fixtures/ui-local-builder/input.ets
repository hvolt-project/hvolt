@LocalBuilder
function localContent() {
  Column() {
    Text("x")
  }
}

@Entry
@Component
struct UiLocalBuilder {
  build() {
    Column() {
      localContent()
    }
  }
}