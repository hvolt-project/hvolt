
function add(a: number, b: number): number {
  return a + b;
}

function callIt(): number {
  return add(1);
}

@Entry
@Component
struct BatchProbe48 {
  build() {
    Column() {
      Text("x")
    }
  }
}
