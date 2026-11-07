@Entry
@Component
struct PropLinkView {
  @State total: number = 0

  build() {
    Column() {
      ChildView({ value: this.total })
      ChildView({ value: this.total })
    }
  }
}

@Component
struct ChildView {
  @Prop value: number = 0

  build() {
    Text(`${this.value}`)
  }
}
