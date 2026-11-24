
enum Color {
  Red = 1,
}

function read(): number {
  return Color.Nope;
}

@Entry
@Component
struct BatchProbe90 {
  build() {
    Column() {
      Text("x")
    }
  }
}
