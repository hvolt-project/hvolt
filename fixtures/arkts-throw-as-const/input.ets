function throwString() {
  throw "boom";
}

const config = { size: 12 } as const;

@Entry
@Component
struct ThrowProbe {
  build() {
    Column() {
      Text(`${config.size}`)
    }
  }
}
