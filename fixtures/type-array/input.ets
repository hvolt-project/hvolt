function arrayMembers() {
  const xs: number[] = [1];
  xs.push("s");
  return xs.notAReal;
}

@Entry
@Component
struct TypeArray {
  build() {
    Column() {
      Text("x")
    }
  }
}
