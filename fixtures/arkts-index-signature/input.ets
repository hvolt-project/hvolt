
interface Dict {
  [key: string]: number;
}

function read(input: Dict): number {
  return input["a"];
}

@Entry
@Component
struct BatchProbe53 {
  build() {
    Column() {
      Text("x")
    }
  }
}
