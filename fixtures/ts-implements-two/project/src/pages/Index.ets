
interface First {
  a: number;
}

interface Second {
  b: number;
}

class Both implements First, Second {
  a: number = 0;
  b: number = 0;
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
