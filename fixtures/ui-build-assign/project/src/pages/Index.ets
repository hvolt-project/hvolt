
@Entry
@Component
struct BatchProbe76c {
  value: number = 0;

  build() {
    Column() {
      this.value = 1;
      Text("x")
    }
  }
}
