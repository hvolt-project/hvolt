@Entry
@Component
struct CommentForms {
  @State ticks: number = 0

  build() {
    Column() {
      // the box
      Column() {
        Text('inner')
      }
      .width('100%')
      .gesture(
        TapGesture({ count: 1 })
          .onAction(() => {
            this.ticks = 1
          })
      )
      .animation({ duration: 50 })
    }
  }
}
