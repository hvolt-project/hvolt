@Component
struct NoBuildMethod {
  message: string = "x";
}

@Entry
@Component
struct UiBuildMethod {
  build() {
    Column() {
      Text("x")
    }
  }
}
