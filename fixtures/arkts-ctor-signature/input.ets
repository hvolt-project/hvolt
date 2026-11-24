
interface Factory {
  new (): object;
}

function build(input: Factory): object {
  return new input();
}

@Entry
@Component
struct BatchProbe53 {
  build() {
    Column() {
      Text("x")
    }
  }
}
