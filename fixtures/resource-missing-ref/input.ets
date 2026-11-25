@Entry
@Component
struct Batch6Probe {
  build() {
    Column() {
      Text($r('app.string.not_exist'))
      Image($rawfile('missing.png'))
    }
  }
}
