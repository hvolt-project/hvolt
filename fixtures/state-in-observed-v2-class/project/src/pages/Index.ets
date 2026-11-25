@ObservedV2
class StateInV2 {
  @State value: number = 1;
}

@Entry
@ComponentV2
struct StateInV2Probe {
  @Local model: StateInV2 = new StateInV2();

  build() {
    Column() {
      Text(`${this.model.value}`)
    }
  }
}
