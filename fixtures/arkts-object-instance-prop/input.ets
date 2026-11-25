
const constructed = new Object();
constructed.added = 1;
const readBack = constructed.added;

@Entry
@Component
struct BatchProbe9 {
  build() {
    Column() {
      Text("x")
    }
  }
}
