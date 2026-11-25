
interface Pair {
  first: number;
  second: number;
}

function read(input: Pair): number {
  const { first } = input;
  return first;
}

@Entry
@Component
struct BatchProbe67 {
  build() {
    Column() {
      Text("x")
    }
  }
}
