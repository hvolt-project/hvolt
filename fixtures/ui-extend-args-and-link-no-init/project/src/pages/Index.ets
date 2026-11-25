@Extend(Text)
function big(size: number) {
  .fontSize(size)
}

@Entry
@Component
struct LinkProbe {
  @Link noInit: number;

  build() {
    Column() {
      Text("x").big(20)
    }
  }
}
