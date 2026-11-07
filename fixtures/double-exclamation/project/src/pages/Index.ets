// Postfix `!!` two-way binding (`PROPERTIES_ADD_DOUBLE_EXCLAMATION` /
// `STYLE_ADD_DOUBLE_EXCLAMATION`). The four spellings the compiler must tell apart:
//   (a) a component property wrapped in an `as` cast  -> left as written;
//   (b) an attribute's first argument, plain `expr!!` -> `{ value, $value }`;
//   (c) the same inside a conditional                 -> left as written;
//   (d) a component property written `expr!!`         -> `{ value, $value }`.
@Entry
@Component
struct DoubleExclaim {
  @State value: number = 1;
  @State isShowMenu: boolean = false;
  @State cond: boolean = true;

  build() {
    Column() {
      Slider({ value: this.value!! as number, min: 0, max: 10 })
        .showSteps(true)
      Slider({ value: this.value!! })
      Text(`${this.value}`)
        .bindPopup(this.isShowMenu!!, { message: 'shown' })
      Text('cond')
        .bindPopup(this.cond ? this.isShowMenu!! : null, { message: 'cond' })
    }
  }
}
