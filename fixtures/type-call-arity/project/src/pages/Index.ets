function takesOne(value: number): number {
  return value;
}

function typedCalls() {
  return takesOne(1, 2);
}

@Entry
@Component
struct TypeCallArity {
  build() {
    Column() {
      Text("x")
    }
  }
}
