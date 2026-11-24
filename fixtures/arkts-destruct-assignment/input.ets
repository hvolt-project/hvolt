class Pair {
  x: number = 1;
  y: number = 2;
}

function swap(values: number[]): void {
  [values[0], values[1]] = [values[1], values[0]];
}

@Entry
@Component
struct DestructProbe {
  build() {
    Column() {
      Text(`${new Pair().x}`)
    }
  }
}
