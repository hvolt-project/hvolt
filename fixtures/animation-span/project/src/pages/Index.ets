@Entry
@Component
struct AnimSpan {
  build() {
    Column() {
      // MIDDLE: animation 在中间
      Flex()
        .backgroundColor(Color.Red)
        .animation({ duration: 100, curve: Curve.Linear })
        .shadow({ radius: 4 })
      // LAST: animation 是最后一个
      Flex()
        .backgroundColor(Color.Blue)
        .animation({ duration: 200, curve: Curve.Linear })
      // FIRST: animation 是第一个
      Flex()
        .animation({ duration: 300, curve: Curve.Linear })
        .backgroundColor(Color.Green)
    }
  }
}
