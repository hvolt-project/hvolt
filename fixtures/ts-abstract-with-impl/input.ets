
abstract class Base {
  abstract read(): number;
}

class Child extends Base {
  read(): number {
    return 1;
  }
}

function make(): Child {
  return new Child();
}

@Entry
@Component
struct BatchProbe89 {
  build() {
    Column() {
      Text("x")
    }
  }
}
