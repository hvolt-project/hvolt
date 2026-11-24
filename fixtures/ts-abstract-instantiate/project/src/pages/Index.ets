
abstract class Base {
  abstract read(): number;
}

function make(): Base {
  return new Base();
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
