@Component
struct NoBuild {
  @State value: string = "";

  render() {
    Column() {
      Text(this.value)
    }
  }
}

@Entry
@Component
struct Batch7Probe {
  @State text: string = "";

  build() {
    Column() {
      TextInput({ text: $$this.text })
        .fontSize(12)
      NoBuild()
    }
  }
}
