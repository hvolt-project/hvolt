
@Extend(Text)
function fancy() {
  .fontSize(16)
}

@Entry
@Component
struct BatchProbe83b {
  build() {
    Column() {
      Text("x").fancy()
    }
  }
}
