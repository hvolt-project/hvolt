class BuilderHolder {
  @Builder
  content() {
    Text("x")
  }
}

@Entry
@Component
struct UiBuilderScope {
  @Builder
  card(title: string) {
    Column() {
      Text(title)
    }
  }

  build() {
    Column() {
      this.card("x")
    }
  }
}