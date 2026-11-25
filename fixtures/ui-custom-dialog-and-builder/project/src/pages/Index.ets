@CustomDialog
struct MyDialog {
  build() {
    Column() {
      Text("x")
    }
  }
}

@Entry
@Component
struct DialogProbe {
  @Builder
  builderWithReturn(): number {
    return 1;
  }

  build() {
    Column() {
      Text("y")
    }
  }
}
