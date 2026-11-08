@Entry
@Component
struct UnusedSource {
  @State label: string = 'reachable';

  build() {
    Column() {
      Text(this.label)
    }
  }
}
