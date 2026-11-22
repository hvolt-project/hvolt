function unionWrong() {
  let u: number | string = 1;
  u = true;
  return u;
}

function unionOk() {
  let u: number | string = 1;
  u = "s";
  return u;
}

function castWrong() {
  const c = "s" as number;
  return c;
}

@Entry
@Component
struct TypeUnion {
  build() {
    Column() {
      Text("x")
    }
  }
}