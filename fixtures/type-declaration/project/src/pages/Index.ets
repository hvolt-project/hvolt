interface Holder {
  count: number;
}

function declWrong(): number {
  const n: number = "s";
  return n;
}

function objectWrong(): Holder {
  return { count: "s" };
}

@Entry
@Component
struct TypeDeclaration {
  build() {
    Column() {
      Text("x")
    }
  }
}