
interface Pair {
  first: number;
  second: number;
}

function read(input: Pair): Pair {
  const copy: Pair = { ...input };
  return copy;
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
