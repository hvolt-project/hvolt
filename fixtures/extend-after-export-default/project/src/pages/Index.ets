// A helper is emitted where its declaration stood. `export default` is written before the
// `@Extend` here, so the generated helper has to come *after* the export — no matter how much
// the file's child blocks widen it. The six constructions below add thirty bytes before both,
// more than the gap between the two declarations, which is what used to make the helper jump
// ahead.
@Entry
@Component
struct ExtendAfterExport {
  build() {
    Column() {
      Text('a')
      Row() {
        Text('b')
      }
      Column() {
        Text('c')
      }
      Row() {
        Text('d')
      }
      Column() {
        Text('e')
      }
      Row() {
        Text('f')
      }
    }
  }
}
export default ExtendAfterExport

@Extend(Text)
function tailPad(pad: number) {
  .padding(pad)
}