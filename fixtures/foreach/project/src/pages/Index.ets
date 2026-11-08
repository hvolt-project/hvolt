@Entry
@Component
struct ListView {
  @State items: string[] = []

  build() {
    Column() {
      ForEach(this.items, (item: string) => {
        Text(item)
      }, (item: string) => item)
    }
  }
}
