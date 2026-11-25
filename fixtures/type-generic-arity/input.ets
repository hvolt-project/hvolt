
function pair<Left, Right>(first: Left, second: Right): Left {
  return first;
}

const one = pair<number>(1);

@Entry
@Component
struct BatchProbe28 {
  build() {
    Column() {
      Text("x")
    }
  }
}
