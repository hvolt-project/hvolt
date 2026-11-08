@Builder
function tag(text?: string) {
  Text(text ?? 'none')
}

@Entry
@Component
struct BuilderOptionalParamPage {
  build() {
    Column() {
      tag('one')
    }
  }
}
