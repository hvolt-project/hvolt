function takesNumber(value: number): number {
  return value;
}

function callWrong() {
  takesNumber("s");
}

function declString() {
  const s: string = 1;
  return s;
}

@Entry
@Component
struct TypeCallArgument {
  build() {
    Column() {
      Text("x")
    }
  }
}
