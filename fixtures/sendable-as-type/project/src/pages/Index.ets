
@Sendable
class SharedBox {
  value: number = 0;
}

function take(box: Sendable): number {
  return 1;
}

@Entry
@Component
struct BatchProbe43 {
  build() {
    Column() {
      Text("x")
    }
  }
}
