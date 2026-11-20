function constAssertion() {
  const kinds = ["a", "b"] as const;
  return kinds.length;
}

@Entry
@Component
struct ArktsAsConst {
  build() {
    Column() {
      Text("x")
    }
  }
}