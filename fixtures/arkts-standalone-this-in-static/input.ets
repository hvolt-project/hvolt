class Tally {
  static total: number = 0;

  static bump(): number {
    return this.total + 1;
  }
}

@Entry
@Component
struct StaticThisCleanProbe {
  build() {
    Column() {
      Text("x")
    }
  }
}
