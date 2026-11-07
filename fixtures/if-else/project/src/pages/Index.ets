@Entry
@Component
struct BranchView {
  @State flag: boolean = false

  build() {
    Column() {
      if (this.flag) {
        Text('on')
      } else {
        Text('off')
      }
    }
  }
}
