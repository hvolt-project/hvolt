@Entry
@ComponentV2
struct ReuseParent {
  @Local n: number = 1;

  build() {
    Column() {
      ReuseKid({ title: `${this.n}` })
    }
  }
}

@ComponentV2
@ReusableV2
struct ReuseKid {
  @Param title: string = '';

  build() {
    Text(this.title)
  }
}
