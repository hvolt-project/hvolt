
class Plain {
  value: number = 1;
}

const fn = Plain.prototype.valueOf;
const applied = Function.prototype.apply;

@Entry
@Component
struct BatchProbe21 {
  build() {
    Column() {
      Text("x")
    }
  }
}
