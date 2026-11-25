class Secret {
  #hidden: number = 1;

  read(): number {
    return this.#hidden;
  }
}

@Entry
@Component
struct ArktsPrivateIdentifiers {
  build() {
    Column() {
      Text("x")
    }
  }
}
