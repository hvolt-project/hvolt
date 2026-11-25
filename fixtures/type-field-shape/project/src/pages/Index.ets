interface Holder {
  count: number;
}

@Entry
@Component
struct FieldShapeProbe {
  @State holder: Holder = {};

  build() {
    Column() {
      Text(`${this.holder.count}`)
    }
  }
}
