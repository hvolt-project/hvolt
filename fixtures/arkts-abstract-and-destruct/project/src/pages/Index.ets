abstract class Base {
  abstract size(): number;
}

function destruct() {
  const holder = { a: 1, b: 2 };
  const { a, b } = holder;
  return a + b;
}

@Entry
@Component
struct AbstractProbe {
  build() {
    Column() {
      Text("x")
    }
  }
}
