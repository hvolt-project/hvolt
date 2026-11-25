
function read(): number {
  const list: number[] = [1, 2];
  return list.nope();
}

@Entry
@Component
struct BatchProbe73 {
  build() {
    Column() {
      Text("x")
    }
  }
}
