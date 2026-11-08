@Entry
@Component
struct SheetView {
  @State shown: boolean = false

  @Builder panel() {
    Text('panel')
  }

  build() {
    Column() {
      Button('open').onClick(() => {
        this.shown = true
      })
    }
    .bindSheet($$this.shown, this.panel(), { detents: [100] })
  }
}
