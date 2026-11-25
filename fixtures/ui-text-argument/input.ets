@Entry
@Component
struct UiTextArgument {
  count: number = 1;

  build() {
    Column() {
      Text(1)
      Text("ok")
      Text(`${this.count}`)
    }
  }
}