@Entry
@ComponentV2
struct V2StateProbe {
  @State value: number = 1;

  build() {
    Column() {
      Text(`${this.value}`)
    }
  }
}
