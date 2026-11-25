
function read(): number {
  const pair: [number, string] = [1, "x"];
  return pair[0];
}

@Entry
@Component
struct BatchProbe86 {
  build() {
    Column() {
      Text("x")
    }
  }
}
