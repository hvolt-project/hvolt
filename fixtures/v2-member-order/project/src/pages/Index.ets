@Entry
@ComponentV2
struct V2Order {
  @Local first: number = 1;

  helper(): number {
    return this.first;
  }

  @Local second: number = 2;

  build() {
    Column() {
      Text(`${this.first}${this.second}`)
    }
  }

  tail(): number {
    return 3;
  }
}
