@Reusable
@Component
struct ReusableChild {
  @State label: string = 'child'

  build() {
    Text(this.label).fontSize(14)
  }
}

@Entry
@Component
struct Index {
  build() {
    Column() {
      ReusableChild({ label: 'a' })
      ReusableChild({ label: 'b' })
    }
  }
}
