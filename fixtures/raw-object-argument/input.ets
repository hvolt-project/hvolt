// `process_component_class.js` `isNeedGetRawObject`: an object-typed state member handed to a
// method call is read through `ObservedObject.GetRawObject`. A member whose declared type is one
// of `process_component_member.js`'s `simpleTypes` — `string`, `number`, `boolean`, an enum — is
// not, and neither is a read that only takes part in a larger expression.
class Options {
  series: string[] = []
}

enum Mode {
  Off = 0,
  On = 1
}

@Entry
@Component
struct Index {
  @State options: Options = new Options()
  @State count: number = 0
  @State mode: Mode = Mode.Off

  build() {
    Column() {
      Text(`${Object.keys(this.options).length}`)
      Text(`${Math.max(this.count, 0)}`)
      Text(`${Math.min(this.mode)}`)
      Text(`${this.options.series.length}`)
    }
  }
}
