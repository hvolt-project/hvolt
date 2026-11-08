@Entry
@Component
struct CustomBuilderProps {
  @State index: number = 0

  @Builder TabBar() {
    Text('tab')
      .fontSize(12)
  }

  build() {
    Column() {
      Tabs() {
        TabContent() {
          Text('first')
        }
        .tabBar(this.TabBar())
      }
    }
  }
}
