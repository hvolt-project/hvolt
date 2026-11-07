@Entry
@Component
struct Index {
  @State message: string = 'Hello Native';

  build() {
    RelativeContainer() {
      Text(this.message)
        .id('HelloNative')
        .fontSize(50)
        .fontWeight(FontWeight.Bold)
        .alignRules({
          center: { anchor: '__container__', align: VerticalAlign.Center },
          middle: { anchor: '__container__', align: HorizontalAlign.Center }
        })
    }
    .height('100%')
    .width('100%')
  }
}
