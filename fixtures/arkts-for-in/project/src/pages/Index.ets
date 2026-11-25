
interface Pair {
  first: number;
}

function read(input: Pair): number {
  let total = 0;
  for (const key in input) {
    total += 1;
  }
  return total;
}

@Entry
@Component
struct BatchProbe73 {
  build() {
    Column() {
      Text("x")
    }
  }
}
