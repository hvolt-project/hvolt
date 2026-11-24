
function callIt(): void {
  console.info(missing);
}

@Entry
@Component
struct BatchProbe50 {
  build() {
    Column() {
      Text("x")
    }
  }
}
