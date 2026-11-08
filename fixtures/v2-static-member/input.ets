@Entry
@ComponentV2
struct V2Static {
  static readonly TAG: string = 'stat';
  @Local count: number = 0;

  build() {
    Column() {
      Text(`${this.count}`)
    }
  }
}
