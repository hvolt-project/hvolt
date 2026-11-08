// A single file may declare both protocols: an `@Component` helper struct next to the
// `@ComponentV2` page. The route must be decided per struct, not per file.
@Component
struct HelperView {
  @Prop label: string = ''

  build() {
    Text(this.label)
  }
}

@Entry
@ComponentV2
struct MixedView {
  @Local count: number = 0

  build() {
    Column() {
      HelperView({ label: `${this.count}` })
    }
  }
}
