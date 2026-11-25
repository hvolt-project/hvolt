interface FactoryShape {
  new (): number;
}

function limitedStdlib(source: Record<string, number>) {
  const frozen = Object.freeze(source);
  return Object.assign({}, frozen);
}

@Entry
@Component
struct ArktsIfaceAndStdlib {
  build() {
    Column() {
      Text(`${limitedStdlib({})}`)
    }
  }
}
