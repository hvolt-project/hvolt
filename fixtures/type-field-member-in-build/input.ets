interface Shape {
  width: number;
}

@Entry
@Component
struct FieldMemberProbe {
  box: Shape = { width: 1 };

  build() {
    Column() {
      Text(`${this.box.missing}`)
    }
  }
}
