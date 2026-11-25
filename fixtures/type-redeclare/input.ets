class Counter {
  value: number = 1;
}

@Entry
@Component
struct TypeRedeclare {
  build() {
    Column() {
      Text(`${new Counter().value}`)
    }
  }
}