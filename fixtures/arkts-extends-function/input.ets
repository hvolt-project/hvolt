
class Base {
  value: number = 1;
}

function free(): number {
  return 1;
}

class Derived extends free {
  extra: number = 2;
}

@Entry
@Component
struct BatchProbe36 {
  build() {
    Column() {
      Text("x")
    }
  }
}
