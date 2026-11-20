@Entry
@Component
struct UiStateDefault {
  @State optional?: number;

  @State counted: number = 1;

  build() {
    Column() {
      Text(`${this.optional}${this.counted}`)
    }
  }
}