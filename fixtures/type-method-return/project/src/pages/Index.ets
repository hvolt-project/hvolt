class Tally {
  count(): number {
    return "s";
  }
}

@Entry
@Component
struct MethodReturnProbe2 {
  build() {
    Column() {
      Text("x")
    }
  }
}
