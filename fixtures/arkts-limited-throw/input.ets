
function read(): number {
  throw "boom";
}

@Entry
@Component
struct BatchProbe85 {
  build() {
    Column() {
      Text("x")
    }
  }
}
