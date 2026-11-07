@Sendable
class Point {
  x: number = 0
  y: number = 0
}

@Sendable
function origin(): Point {
  return new Point()
}

@Entry
@Component
struct Index {
  build() {
    Column() {
      Text('sendable')
        .fontSize(16)
    }
  }
}
