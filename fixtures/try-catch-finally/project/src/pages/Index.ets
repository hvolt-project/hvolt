
function read(): number {
  try {
    return 1;
  } catch (e) {
    return 0;
  }
}

@Entry
@Component
struct BatchProbe85 {
  build() {
    Column() {
      Text("x")
    }
  }
}
