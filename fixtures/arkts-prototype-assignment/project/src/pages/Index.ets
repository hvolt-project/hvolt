
function plain(): number {
  return 1;
}

const proto = Function.prototype;
const bound = plain.bind(null);
const applied = plain.apply(null, []);
const called = plain.call(null);

@Entry
@Component
struct BatchProbe12 {
  build() {
    Column() {
      Text("x")
    }
  }
}
