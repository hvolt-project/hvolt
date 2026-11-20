function noImplicit(param) {
  return param;
}

@Entry
@Component
struct TypeImplicitAny {
  build() {
    Column() {
      Text("x")
    }
  }
}
