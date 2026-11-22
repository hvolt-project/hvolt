@Builder function card(title: string) {
  Column() {
    Text(title)
  }
}

@Entry
@Component
struct GlobalBuilderProbe {
  build() {
    Column() {
      card(1)
    }
  }
}
