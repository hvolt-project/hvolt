interface Holder {
  count: number;

  label: string;
}

function declPartial() {
  const h: Holder = { count: 1 };
  const ok: Holder = { count: 1, label: "x" };
  return `${h.label}${ok.label}`;
}

@Entry
@Component
struct TypeMissingPartial {
  build() {
    Column() {
      Text("x")
    }
  }
}