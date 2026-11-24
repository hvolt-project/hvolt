class PlainBase {
  value: number = 0;
}

@Sendable
class SharedDerived extends PlainBase {
  extra: number = 1;
}

@Entry
@Component
struct SendableInheritProbe {
  build() {
    Column() {
      Text(`${new SharedDerived().extra}`)
    }
  }
}
