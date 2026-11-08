@Entry
@Component
struct MemberShapes {
  private scroller = this.getUIContext()
  private cancel?: () => void
  @Prop noDefault: string
  @Prop withDefault: string = ''
  @BuilderParam extra?: () => void
  @State count: number = 0

  build() {
    Column() {
      Text(`${this.count} ${this.withDefault}`)
    }
  }

  @Builder
  badge(text: string) {
    Text(text)
  }
}
