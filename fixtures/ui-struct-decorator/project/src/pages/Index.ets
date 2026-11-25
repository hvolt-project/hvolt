struct StructWithoutDecorator {
  build() {
    Column() {
      Text("x")
    }
  }
}

@Entry
@Component
struct RealEntry {
  build() {
    Column() {
      Text("y")
    }
  }
}