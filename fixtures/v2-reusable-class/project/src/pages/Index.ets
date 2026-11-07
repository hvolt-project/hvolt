@Entry
@ComponentV2
struct ReusePage {
  @Local count: number = 1;

  build() {
    Column() {
      Text(`${this.count}`)
    }
  }
}

// Declared here, instantiated from another file at run time: the class and the file's reflect
// helper are emitted without a call site in this file.
@ComponentV2
@ReusableV2
export struct ReuseCard {
  @Param title: string = '';

  build() {
    Text(this.title)
  }
}
