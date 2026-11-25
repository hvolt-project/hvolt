function assignWrong() {
  let x = 1;
  x = "s";
  return x;
}

function arrayWrong() {
  const arr: number[] = ["a"];
  return arr.length;
}

@Entry
@Component
struct TypeAssign {
  build() {
    Column() {
      Text("x")
    }
  }
}