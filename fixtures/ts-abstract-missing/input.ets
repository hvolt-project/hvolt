
abstract class Base {
  abstract read(): number;
}

class Child extends Base {
  value: number = 0;
}

@Entry
@Component
struct BatchProbe88 {
  build() {
    Column() {
      Text("x")
    }
  }
}
