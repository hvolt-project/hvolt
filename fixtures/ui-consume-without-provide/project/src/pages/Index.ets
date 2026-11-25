@Entry
@Component
struct ConsumeProbe {
  @Consume missing: number;

  build() {
    Column() {
      Text(`${this.missing}`)
    }
  }
}
