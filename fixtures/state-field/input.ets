@Entry
@Component
struct CounterView {
  @State count: number = 0

  build() {
    Column() {
      Text(`${this.count}`)
    }
  }
}
