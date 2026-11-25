
const items = [{ value: 1 }, { value: 2 }];
const nested = [[1], [2]];
const mixed = [1, "a"];

@Entry
@Component
struct NonInferrableProbe {
  build() {
    Column() {
      Text("x")
    }
  }
}
