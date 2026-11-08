@Entry
@Component
struct AttrForms {
  @State open: boolean = false
  @State progress: number = 0

  @Builder panel() {
    Text('panel')
  }

  build() {
    Column() {
      Text('plain')
        .fontSize(16)
        .fontColor(Color.Red)
      Text('sheet')
        .bindSheet($$this.open, this.panel(), { detents: [100] })
      Column() {
        Text('drag')
      }
      .width('100%')
      .gesture(
        PanGesture({ fingers: 1 })
          .onActionStart(() => {
            this.progress = 1
          })
          .onActionEnd(() => {
            this.progress = 0
          })
      )
      .animation({ duration: 100 })
    }
  }
}
