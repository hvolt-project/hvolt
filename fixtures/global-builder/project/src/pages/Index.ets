@Builder
function badge(text: string) {
  Text(text)
    .fontSize(12)
}

@Builder
export function pair(left: string, right: string) {
  Row() {
    Text(left)
    Text(right)
  }
}

@Entry
@Component
struct GlobalBuilderPage {
  build() {
    Column() {
      badge('one')
      pair('two', 'three')
    }
  }
}
