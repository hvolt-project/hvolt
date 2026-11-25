
@Sendable
class ShareHolder {
  ready!: number;
  plain: number = 0;
}

@Entry
@Component
struct BatchProbe5 {
  build() {
    Column() {
      Text("x")
    }
  }
}
