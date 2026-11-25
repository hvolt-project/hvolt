@Entry
@Component
struct PropOnClassProbe {
  @Prop linkValue: number = 0;

  build() {
    Column() {
      Text(`${this.linkValue}`)
    }
  }
}
