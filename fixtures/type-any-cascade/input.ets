function anyCascade() {
  const text: string = "x";
  const bad = text.notARealMethod();
  return `${bad}`;
}

@Entry
@Component
struct TypeAnyCascade {
  build() {
    Column() {
      Text("x")
    }
  }
}
