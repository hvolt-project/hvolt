@Entry
@Component
struct BuilderView {
  @Builder label(text: string) {
    Text(text)
  }

  build() {
    Column() {
      this.label('hello')
    }
  }
}
