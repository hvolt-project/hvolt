function anyUse() {
  const x: any = 1;
  const y = 2 as any;
  return x + y;
}

@Entry
@Component
struct AnyProbe {
  build() {
    Column() {
      Text("x")
    }
  }
}
