@Entry
@Component
struct WatchView {
  @State count: number = 0
  @Watch('onCountChange') @State label: string = 'zero'

  onCountChange(): void {
    this.label = `${this.count}`
  }

  build() {
    Column() {
      Text(this.label)
    }
  }
}
