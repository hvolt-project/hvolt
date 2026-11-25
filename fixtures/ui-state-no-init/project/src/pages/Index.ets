
@Entry
@Component
struct StateProbe {
  @State counter: number;

  build() {
    Column() {
      Text(`${this.counter}`)
    }
  }
}
