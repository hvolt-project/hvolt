
interface Named {
  name: string;
  size: number;
}

class Item implements Named {
  name: string = "x";
}

@Entry
@Component
struct BatchProbe70 {
  build() {
    Column() {
      Text("x")
    }
  }
}
