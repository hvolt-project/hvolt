
class Holder {
  readonly tag: string = "x";
}

function write(input: Holder): void {
  input.tag = "y";
}

@Entry
@Component
struct BatchProbe49 {
  build() {
    Column() {
      Text("x")
    }
  }
}
