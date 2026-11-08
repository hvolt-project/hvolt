@Entry
@Component
struct ConstructorView {
  label: string = 'ready'
  @State count: number = 0

  build() {
    Column() {
      Text(this.label)
    }
  }
}