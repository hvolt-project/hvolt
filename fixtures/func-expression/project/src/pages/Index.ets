@Entry
@Component
struct Index {
  private value!: number;

  private tick = function () {
    return 1;
  };

  build() {
    Column() {
      Text(`${this.value}`)
    }
  }
}
