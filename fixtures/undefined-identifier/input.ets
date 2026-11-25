@Entry
@Component
struct UndefinedIdentifierPage {
  build() {
    Column() {
      Text(notDeclaredAnywhere)
    }
  }
}
