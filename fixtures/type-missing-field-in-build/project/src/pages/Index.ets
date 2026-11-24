interface Shape {
  width: number;
  height: number;
}

function describe(shape: Shape): string {
  return `${shape.width}`;
}

@Entry
@Component
struct MissingFieldProbe {
  build() {
    Column() {
      Text(describe({ width: 1 }))
    }
  }
}
