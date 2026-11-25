function multiply(a: number, b: number): number {
  return a * b;
}

const one = multiply(1);
const two = multiply("x", 1);

@Entry
@Component
struct ArgCountProbe {
  build() {
    Column() {
      Text(`${one}${two}`)
    }
  }
}
