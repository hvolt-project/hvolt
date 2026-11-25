
class Holder {
  value: number = 0;
}

interface Reader extends Holder {
  read(): number;
}

@Entry
@Component
struct BatchProbe88 {
  build() {
    Column() {
      Text("x")
    }
  }
}
