class Base {
  value: number = 1;
}

class Derived extends Base {}

function identity<T>(value: T): T {
  return value;
}

@Entry
@Component
struct InferProbe {
  build() {
    Column() {
      Text(`${identity(1)}${new Derived().value}`)
    }
  }
}
