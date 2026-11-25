
class Box {
  constructor(a: number, b: string) {
    this.first = a;
    this.second = b;
  }

  first: number = 0;
  second: string = "";
}

function make(): Box {
  return new Box(1, "x", true);
}

@Entry
@Component
struct BatchProbe58 {
  build() {
    Column() {
      Text("x")
    }
  }
}
