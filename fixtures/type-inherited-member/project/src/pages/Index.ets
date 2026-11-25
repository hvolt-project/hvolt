class Base {
  value: number = 1;
}

class Derived extends Base {
  extra: string = "x";
}

@Entry
@Component
struct ExtendsProbe {
  build() {
    Column() {
      Text(`${new Derived().extra}${new Base().value}`)
    }
  }
}
