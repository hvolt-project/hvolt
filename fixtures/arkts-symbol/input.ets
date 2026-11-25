
function pick(): number {
  const tag = Symbol("x");
  return tag.description.length;
}

@Entry
@Component
struct BatchProbe65 {
  build() {
    Column() {
      Text("x")
    }
  }
}
