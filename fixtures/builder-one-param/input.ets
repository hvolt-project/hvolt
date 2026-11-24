
@Builder
function card(title: string) {
  Column() {
    Text(title)
  }
}

@Entry
@Component
struct BatchProbe78b {
  build() {
    Column() {
      card("t")
    }
  }
}
