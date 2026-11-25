
function build(): number[] {
  const base: number[] = [1, 2];
  const copy: number[] = [...base];
  return copy;
}

@Entry
@Component
struct BatchProbe67 {
  build() {
    Column() {
      Text("x")
    }
  }
}
