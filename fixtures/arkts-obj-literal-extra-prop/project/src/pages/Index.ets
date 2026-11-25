
class Config {
  name: string = "";
}

const config: Config = { name: "a", extra: 1 };

const arrays = [[1, 2], [3]];

@Entry
@Component
struct BatchProbe3 {
  build() {
    Column() {
      Text("x")
    }
  }
}
