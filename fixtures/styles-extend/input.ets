@Entry
@Component
struct StyleView {
  @Styles box() {
    .width(10)
    .height(10)
  }

  build() {
    Column() {
      Text('x').box()
    }
  }
}
