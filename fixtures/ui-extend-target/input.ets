@Extend(Nope) function fancy() {
  .fontSize(12)
}

@Entry
@Component
struct ExtendTargetProbe {
  build() {
    Column() {
      Text("x").fancy()
    }
  }
}
