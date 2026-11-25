@Entry
@Component
struct Batch8Probe {
  @State value: string = "";

  build() {
    Column() {
      TextInput({ text: this.value })
        .onChange((v: string) => {
          this.value = v;
        })
      Progress({ value: 10, total: 100 })
    }
  }
}
