function withArgs(a: number, ...rest: number[]) {
  return a + rest.length;
}

function spreadCall(numbers: number[]) {
  return withArgs(...numbers);
}

function defaultParam(a: number = 1) {
  return a;
}

function useArguments() {
  return arguments.length;
}

@Entry
@Component
struct Batch2Probe {
  build() {
    Column() {
      Text(`${spreadCall([1, 2]) + defaultParam()}`)
    }
  }
}
