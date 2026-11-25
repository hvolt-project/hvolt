interface Point {
  x: number;

  y: number;
}

function destructParam({ x, y }: Point) {
  return x + y;
}

function destructDeclarations(values: number[]) {
  const [first, second] = values;
  return `${first}${second}`;
}

@Entry
@Component
struct ArktsDestructuring {
  build() {
    Column() {
      Text("x")
    }
  }
}