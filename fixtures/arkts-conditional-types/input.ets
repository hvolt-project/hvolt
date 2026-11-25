type Pair = [number, string];

type Keys = keyof Pair;

function tupleUse(pair: Pair) {
  return pair[0];
}

function literalType(mode: "on" | "off") {
  return mode;
}

type Conditional<T> = T extends number ? string : number;

@Entry
@Component
struct Batch3Probe {
  build() {
    Column() {
      Text(`${tupleUse([1, "a"])}`)
    }
  }
}
