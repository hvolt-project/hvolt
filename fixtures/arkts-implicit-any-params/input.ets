function add(a, b) {
  return a + b;
}

@Entry
@Component
struct ImplicitAnyProbe {
  build() {
    Column() {
      Text(`${add(1, 2)}`)
    }
  }
}
