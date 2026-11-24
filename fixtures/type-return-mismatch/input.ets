
function compute(): number {
  return 1;
}

const assigned: string = compute();

@Entry
@Component
struct BatchProbe27 {
  build() {
    Column() {
      Text("x")
    }
  }
}
