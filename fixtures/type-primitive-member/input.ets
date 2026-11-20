function primitiveMembers() {
  const text: string = "x";
  const size: number = 1;
  const width = text.length;
  const padded = text.padStart(2);
  text.notARealMethod();
  return `${width}${padded}${size}`;
}

@Entry
@Component
struct TypePrimitiveMember {
  build() {
    Column() {
      Text("x")
    }
  }
}
