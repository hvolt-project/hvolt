
class Util {
  identity<T>(input: T): T {
    return input;
  }
}

function read(): number {
  const util = new Util();
  return util.identity(1);
}

@Entry
@Component
struct BatchProbe87 {
  build() {
    Column() {
      Text("x")
    }
  }
}
