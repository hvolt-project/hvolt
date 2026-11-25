
@Sendable
function shared(): number {
  return 1;
}

shared.extra = 2;

@Entry
@Component
struct BatchProbe42 {
  build() {
    Column() {
      Text("x")
    }
  }
}
