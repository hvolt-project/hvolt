@Entry
@Component
struct LinkParent {
  @State shared: number = 0

  build() {
    Column() {
      LinkChild({ shared: $shared })
    }
  }
}

@Component
struct LinkChild {
  @Link shared: number

  build() {
    Text(`${this.shared}`)
  }
}
