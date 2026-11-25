class Counter {
  static total: number = 0;

  static bump(): number {
    return this.total + 1;
  }
}

@Entry
@Component
struct StaticThisProbe {
  build() {
    Column() {
      Text(`${Counter.bump()}`)
    }
  }
}
