
interface Shape {
  area(): number;
}

class Square extends Shape {
  area(): number {
    return 1;
  }
}

@Entry
@Component
struct BatchProbe17 {
  build() {
    Column() {
      Text("x")
    }
  }
}
