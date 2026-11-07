@Entry
@ComponentV2
struct V2LocalBuilder {
  @Local label: string = 'l';

  @LocalBuilder
  chip(t: string) {
    Text(t)
  }

  build() {
    Column() {
      this.chip(this.label)
    }
  }
}
