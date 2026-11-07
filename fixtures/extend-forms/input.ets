@Extend(Text) function bold(size: number) {
  .fontWeight(FontWeight.Bold)
  .fontSize(size)
}

@Extend(Text)
function padded(pad: number) {
  .padding(pad)
}

@Entry
@Component
struct ExtendForms {
  build() {
    Column() {
      Text('a').bold(16)
      Text('b').padded(4)
    }
  }
}
