@Entry
@Component
struct BuildReturnProbe {
  constructor() {
    super();
  }

  build() {
    Column() {
      Text("x")
    }
    return 1;
  }
}
