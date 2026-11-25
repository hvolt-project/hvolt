
const key = "value";

@Sendable
class SharedComputed {
  [key]: number = 0;
}

@Entry
@Component
struct BatchProbe40 {
  build() {
    Column() {
      Text("x")
    }
  }
}
