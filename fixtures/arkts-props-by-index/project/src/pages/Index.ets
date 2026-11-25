interface Holder {
  count: number;
}

function byIndex() {
  const h: Holder = { count: 1 };
  const v = h["count"];
  return v;
}

@Entry
@Component
struct IndexAccessProbe {
  build() {
    Column() {
      Text("x")
    }
  }
}
