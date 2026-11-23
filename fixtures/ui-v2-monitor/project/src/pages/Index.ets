@ObservedV2
class Model {
  @Trace size: number = 1;
}

@Entry
@ComponentV2
struct Batch5Probe {
  @Local model: Model = new Model();
  @Monitor("model.size")
  onSize() {
    return 0;
  }

  build() {
    Column() {
      Text(`${this.model.size}`)
    }
  }
}
