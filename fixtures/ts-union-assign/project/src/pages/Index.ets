
function read(): number {
  let value: number | string = 1;
  value = "x";
  return 1;
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
