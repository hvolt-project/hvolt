class Vertex {
  constructor(private x: number) {}
}

function spreadObject(seed: Record<string, number>) {
  const copy = { ...seed };
  return copy;
}

@Entry
@Component
struct CtorPropProbe {
  build() {
    Column() {
      Text("x")
    }
  }
}
