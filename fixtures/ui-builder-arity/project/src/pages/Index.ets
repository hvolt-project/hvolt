@Entry
@Component
struct UiBuilderArity {
  @Builder
  card(title: string) {
    Column() {
      Text(title)
    }
  }

  build() {
    Column() {
      this.card()
      this.card("a", "b")
      this.card("ok")
    }
  }
}