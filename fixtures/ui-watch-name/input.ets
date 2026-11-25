@Entry
@Component
struct UiWatchName {
  @State @Watch('onMissing') counter: number = 0;

  @State @Watch('onPresent') total: number = 0;

  onPresent() {
    this.total += 1;
  }

  build() {
    Column() {
      Text(`${this.counter}${this.total}`)
    }
  }
}