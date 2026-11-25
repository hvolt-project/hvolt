type Names = Record<string, number>;

function mapped() {
  const n: Names = { a: "s" };
  return n;
}

@Entry
@Component
struct MappedTypeProbe {
  build() {
    Column() {
      Text("x")
    }
  }
}
