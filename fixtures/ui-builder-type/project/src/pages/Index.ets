@Entry
@Component
struct BuilderTypeProbe {
  @Builder
  card(title: string) {
    Column() {
      Text(title)
    }
  }

  build() {
    Column() {
      this.card(1)
    }
  }
}
