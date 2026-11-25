type Pair = [number, string];
type Either = string | number;

function takesEither(value: Either): string {
  return `${value}`;
}

@Entry
@Component
struct TupleProbe {
  build() {
    Column() {
      Text(`${takesEither(1)}`)
    }
  }
}
