
const marker = 1;

@marker
class Decorated {
  value: number = 1;
}

@Entry
@Component
struct BatchProbe37 {
  build() {
    Column() {
      Text("x")
    }
  }
}
