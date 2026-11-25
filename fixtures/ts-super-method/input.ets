
class Base {
  read(): number {
    return 1;
  }
}

class Child extends Base {
  read(): number {
    return super.read() + 1;
  }
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
