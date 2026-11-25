
class Box {
  value: number = 1;
}

const box = new Box();
const good = box.value;
const bad = box.notThere;

@Entry
@Component
struct BatchProbe25 {
  build() {
    Column() {
      Text("x")
    }
  }
}
