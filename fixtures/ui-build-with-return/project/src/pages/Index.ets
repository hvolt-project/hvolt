
@Entry
@Component
struct BuildReturnProbe {
  build(): number {
    Column() {
      Text("x")
    }
    return 1;
  }
}
