
function read(): number {
  const list = [1, 2] as const;
  return list.length;
}

@Entry
@Component
struct BatchProbe90 {
  build() {
    Column() {
      Text("x")
    }
  }
}
