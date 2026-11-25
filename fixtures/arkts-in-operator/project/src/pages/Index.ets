
interface Box {
  value: number;
}

function read(input: Box): boolean {
  return ("value" in input);
}

@Entry
@Component
struct BatchProbe66 {
  build() {
    Column() {
      Text("x")
    }
  }
}
