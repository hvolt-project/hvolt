@Entry
@Component
struct ProvideParent {
  @Provide('theme') value: string = 'light'

  build() {
    Column() {
      ConsumeChild()
    }
  }
}

@Component
struct ConsumeChild {
  @Consume('theme') theme: string

  build() {
    Text(this.theme)
  }
}
