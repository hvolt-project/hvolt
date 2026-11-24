
@Builder
function card(title: string, value: number) {
  Column() {
    Text(title)
    Text(`${value}`)
  }
}

@Entry
@Component
struct BatchProbe78a {
  build() {
    Column() {
      card("t", 1)
    }
  }
}
