@ComponentV2
struct V2StateProbe {
  @State value: number = 0;

  build() {
    Column() {
      Text(`${this.value}`)
    }
  }
}

@Entry
@Component
struct EntryProbe {
  build() {
    Column() {
      Text("x")
    }
  }
}
