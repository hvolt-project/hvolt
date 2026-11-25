class PropDecl {
  constructor(public label: string) {}
}

function angleBracket(value: number) {
  const widened = <number>value;
  return widened;
}

@Entry
@Component
struct ArktsCtorAndCasts {
  build() {
    Column() {
      Text(`${angleBracket(1)}`)
    }
  }
}
