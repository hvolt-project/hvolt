@Entry
@Component
struct Index {
  @State message: string = 'Hello';

  build() {
    Column() {
      Text(this.message)
    }
    .width('100%')
  }
}
