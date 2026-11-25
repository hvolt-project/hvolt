
class Inner {
  flag: boolean = false;
}

class Outer {
  inner: Inner = new Inner();
}

const outer = new Outer();
const read = outer.inner.flag;
const missing = outer.inner.notThere;

@Entry
@Component
struct BatchProbe26 {
  build() {
    Column() {
      Text("x")
    }
  }
}
