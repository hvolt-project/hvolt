interface Holder {
  count: number;
}

function declEmpty() {
  const h: Holder = {};
  const ok: Holder = { count: 1 };
  return h.count + ok.count;
}

@Entry
@Component
struct TypeMissingMember {
  build() {
    Column() {
      Text("x")
    }
  }
}
