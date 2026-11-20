@Entry
@Component
struct StatementsInBuild {
  build() {
    const value = 1;
  }
}

@Entry
@Component
struct SecondEntry {
  build() {
    Column() {
      Text("x")
    }
  }
}
