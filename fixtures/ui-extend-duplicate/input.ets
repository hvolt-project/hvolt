@Extend(Text) function fancy() {
  .fontSize(12)
}

@Extend(Text) function fancy() {
  .fontSize(14)
}

@Entry
@Component
struct UiExtendDuplicate {
  build() {
    Column() {
      Text("x").fancy()
    }
  }
}