@Entry
@Component
struct UiStaticState {
  @State static counter: number = 0;

  @Link linked: number = 1;

  build() {
    Column() {
      Text(`${this.counter}${this.linked}`)
    }
  }
}