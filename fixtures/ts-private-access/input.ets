
class Holder {
  private secret: number = 1;
}

function read(input: Holder): number {
  return input.secret;
}

@Entry
@Component
struct BatchProbe71 {
  build() {
    Column() {
      Text("x")
    }
  }
}
