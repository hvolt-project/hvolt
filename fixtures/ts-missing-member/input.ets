
interface Pair {
  first: number;
  second: number;
}

function read(input: Pair): number {
  return input.third;
}

@Entry
@Component
struct BatchProbe48 {
  build() {
    Column() {
      Text("x")
    }
  }
}
