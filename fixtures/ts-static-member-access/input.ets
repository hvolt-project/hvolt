
class Holder {
  static total: number = 0;
}

function read(input: Holder): number {
  return input.total;
}

@Entry
@Component
struct BatchProbe72 {
  build() {
    Column() {
      Text("x")
    }
  }
}
