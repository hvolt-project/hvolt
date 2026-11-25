class PlainState {
  @State
  value: number = 1;
}

@Entry
@Component
struct UiRestrictedChildren {
  build() {
    Column() {
      Text("parent") {
        Text("child")
      }
    }
  }
}
