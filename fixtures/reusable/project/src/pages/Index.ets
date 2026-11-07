@Entry
@Reusable
@Component
struct ReusableCard {
  @State count: number = 0
  @Prop label: string = 'card'

  aboutToAppear(): void {
    this.count = 0
  }

  build() {
    Column() {
      Text(this.label)
    }
  }
}
