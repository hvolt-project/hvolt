
function build(): number[] {
  const list: number[] = [1, "two"];
  return list;
}

@Entry
@Component
struct BatchProbe56 {
  build() {
    Column() {
      Text("x")
    }
  }
}
