
interface Base {
  first: number;
}

interface Derived extends Base {
  second: number;
}

function read(input: Derived): number {
  return input.first + input.second;
}

@Entry
@Component
struct BatchProbe89 {
  build() {
    Column() {
      Text("x")
    }
  }
}
