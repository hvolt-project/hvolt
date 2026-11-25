@Entry
@Component
struct UiStylesArguments {
  @Styles styledColumn(width: number) {
    .width(width)
  }

  build() {
    Column() {
      Text("x")
    }
    .styledColumn(100)
  }
}