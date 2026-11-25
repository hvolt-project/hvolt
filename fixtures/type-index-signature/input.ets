interface Bag {
  [key: string]: number;
}

function indexWrong() {
  const b: Bag = { a: "s" };
  return b;
}

@Entry
@Component
struct IndexSigProbe {
  build() {
    Column() {
      Text("x")
    }
  }
}
