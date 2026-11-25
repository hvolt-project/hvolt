
@Extend(Text)
function fancy(color: string) {
  .fontSize(16)
}

@Entry
@Component
struct BatchProbe83c {
  build() {
    Column() {
      Text("x").fancy("red")
    }
  }
}
