interface Bag {
  a: number;
  b: number;
}

function pick(bag: Bag, key: string): number {
  return bag[key];
}

function viaCall(bag: Bag, key: string): number {
  return Math.max(bag[key], 0);
}

@Entry
@Component
struct IndexProbe2 {
  build() {
    Column() {
      Text(`${pick({ a: 1, b: 2 }, "a")}${viaCall({ a: 1, b: 2 }, "b")}`)
    }
  }
}
