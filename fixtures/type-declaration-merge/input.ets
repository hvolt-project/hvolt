interface Bag {
  a: number;
}

interface Bag {
  b: string;
}

function merged() {
  const x: Bag = { a: 1, b: "s" };
  return x;
}

@Entry
@Component
struct MergeProbe {
  build() {
    Column() {
      Text("x")
    }
  }
}
