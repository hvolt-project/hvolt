class Point {
  x: number = 0;
}

@Entry
@Component
struct RedeclareProbe {
  build() {
    Column() {
      Text(`${new Point().x}`)
    }
  }
}
