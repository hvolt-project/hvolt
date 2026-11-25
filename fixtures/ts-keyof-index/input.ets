
interface Box {
  value: number;
}

function read(input: Box, key: keyof Box): number {
  return input[key];
}

@Entry
@Component
struct BatchProbe86 {
  build() {
    Column() {
      Text("x")
    }
  }
}
