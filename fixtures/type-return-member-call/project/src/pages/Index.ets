function funcCalls(target: (value: number) => number) {
  return target.bind(null);
}

@Entry
@Component
struct TypeReturnMemberCall {
  build() {
    Column() {
      Text("x")
    }
  }
}
