interface Holder {
  count: number;
}

function declExcess() {
  const h: Holder = { count: 1, extra: "x" };
  return h.count;
}

@Entry
@Component
struct ExcessProbe {
  build() {
    Column() {
      Text("x")
    }
  }
}
