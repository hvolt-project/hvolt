@Entry
@Component
struct WatchTargetProbe {
  @State @Watch("missingHandler") value: number = 0;

  build() {
    Column() {
      Text(`${this.value}`)
    }
  }
}
