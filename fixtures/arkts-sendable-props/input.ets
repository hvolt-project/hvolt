class PlainThing {
  value: number = 0;
}

@Sendable
class BadShare {
  plain: PlainThing = new PlainThing();
  list: number[] = [];
}

@Entry
@Component
struct SendableFieldProbe {
  build() {
    Column() {
      Text(`${new BadShare().plain.value}`)
    }
  }
}
