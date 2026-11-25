class Ticker {
  count: number = 0;

  bump(): number {
    this.count += 1;
    return "s";
  }
}

@Entry
@Component
struct TypeFieldAssign {
  @State counter: number = 0;

  update() {
    this.counter = "s";
  }

  build() {
    Column() {
      Text(`${this.counter}`)
    }
  }
}