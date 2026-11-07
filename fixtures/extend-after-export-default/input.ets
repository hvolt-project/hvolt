// A helper is emitted where its declaration stood. `export default` is written before the
// `@Extend` here, so the generated helper has to come *after* the export — no matter how much
// the file's child blocks widen it. The eleven constructions below add fifty-five bytes before
// both, well past the thirty-four byte gap between the two declarations, which is why the helper
// used to jump ahead of the export.
@Entry
@Component
struct ExtendAfterExport {
  build() {
    Column() {
      Row() {
        Text('a')
      }
      Column() {
        Text('b')
      }
      Row() {
        Text('c')
      }
      Column() {
        Text('d')
      }
      Row() {
        Text('e')
      }
      Column() {
        Text('f')
      }
      Row() {
        Text('g')
      }
      Column() {
        Text('h')
      }
      Row() {
        Text('i')
      }
      Column() {
        Text('j')
      }
    }
  }
}
export default ExtendAfterExport

@Extend(Text)
function tailPad(pad: number) {
  .padding(pad)
}