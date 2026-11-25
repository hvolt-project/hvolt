
export namespace Outer {
  export const value = 1;
}

namespace Inner {
  const local = 2;
}

const asObject = Inner;

@Entry
@Component
struct BatchProbe {
  build() {
    Column() {
      Text("x")
    }
  }
}
