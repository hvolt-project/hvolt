
interface Named {
  name: string;
}

abstract class Base implements Named {
  abstract name: string;
}

@Entry
@Component
struct BatchProbe89 {
  build() {
    Column() {
      Text("x")
    }
  }
}
