
const outside = 1;

@Sendable
function shared(): number {
  return outside;
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
