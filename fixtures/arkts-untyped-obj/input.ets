
interface Pair {
  first: number;
}

function read(): number {
  const value = { first: 1 };
  return value.first;
}

@Entry
@Component
struct BatchProbe90 {
  build() {
    Column() {
      Text("x")
    }
  }
}
