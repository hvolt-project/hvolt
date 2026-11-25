
@Entry
@Component
struct BatchProbe79c {
  @Link count: number;

  build() {
    Column() {
      Text(`${this.count}`)
    }
  }
}
