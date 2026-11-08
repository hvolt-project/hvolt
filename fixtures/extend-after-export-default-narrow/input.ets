// The same shape with a single construction: five bytes of widening, nowhere near the forty byte
// gap between `export default` and the `@Extend`. The helper lands behind the export here even
// with the coordinate mix its sibling pins, so the two fixtures together separate "the rule is
// source order" from "the widening moved the offset".
@Entry
@Component
struct ExtendAfterExportNarrow {
  build() {
    Column() {
      Text('a')
    }
  }
}
export default ExtendAfterExportNarrow

@Extend(Text)
function tailPad(pad: number) {
  .padding(pad)
}