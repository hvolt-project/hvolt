@Entry
@ComponentV2
struct V2FieldModifiers {
  plain: number = 3;
  @Require @Param required: string = '';

  build() {
    Column() {
      Text(`${this.plain}${this.required}`)
    }
  }
}
