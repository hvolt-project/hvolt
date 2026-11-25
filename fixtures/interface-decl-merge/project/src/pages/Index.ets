
interface Shape {
  width: number;
}

interface Shape {
  height: number;
}

function area(input: Shape): number {
  return input.width * input.height;
}

@Entry
@Component
struct BatchProbe63 {
  build() {
    Column() {
      Text("x")
    }
  }
}
