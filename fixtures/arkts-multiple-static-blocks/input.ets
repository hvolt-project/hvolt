class StaticBlocks {
  static {
    const a = 1;
  }

  static {
    const b = 2;
  }
}

@Entry
@Component
struct ImportAssertProbe {
  build() {
    Column() {
      Text("x")
    }
  }
}
