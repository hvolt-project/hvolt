@Entry
@Component
struct UiV2Decorators {
  @Param value: number = 1;

  @Local local: number = 2;

  build() {
    Column() {
      Text(`${this.value}${this.local}`)
    }
  }
}