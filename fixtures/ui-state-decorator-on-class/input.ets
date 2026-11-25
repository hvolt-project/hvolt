class PlainDecorators {
  @Prop
  propValue: number = 1;

  @Link
  linkValue: number = 2;

  @Provide
  provideValue: number = 3;
}

@Entry
@Component
struct StateDecorators {
  build() {
    Column() {
      Text("x")
    }
  }
}
