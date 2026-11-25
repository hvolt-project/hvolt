@Entry
@Component
struct Index {
  private count: number = "three";

  private payload: any = undefined;

  build() {
    Column() {
      Text(`${this.count}`)
    }
  }
}
