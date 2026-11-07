@Entry
@Component
struct BuilderCalls {
  @State label: string = 'b'

  @Builder
  Badge() {
    ForEach(['badge'], (item: string) => {
      Text(item)
    })
  }

  @Builder
  Panel() {
    Column() {
      this.Badge()
    }
  }

  build() {
    Column() {
      this.Panel()
    }
  }
}
