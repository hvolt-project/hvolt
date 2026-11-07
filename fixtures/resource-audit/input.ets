@Entry
@Component
struct ResourceAudit {
  @State caption: string = 'res'

  build() {
    Column() {
      Text($r('app.string.module_desc'))
      Image($r('app.media.startIcon'))
      Text($r('sys.color.ohos_id_color_text_primary'))
      Text(this.caption).fontColor($r('app.color.start_window_background'))
    }
  }
}
