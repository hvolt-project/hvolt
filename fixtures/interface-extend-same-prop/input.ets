
interface Base {
  value: number;
}

interface Derived extends Base {
  value: string;
}

@Entry
@Component
struct BatchProbe47 {
  build() {
    Column() {
      Text("x")
    }
  }
}
