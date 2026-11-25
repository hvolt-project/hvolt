
@Sendable
class SharedBox {
  value: number = 0;
}

const made: SharedBox = { value: 1 };

@Entry
@Component
struct BatchProbe39 {
  build() {
    Column() {
      Text("x")
    }
  }
}
