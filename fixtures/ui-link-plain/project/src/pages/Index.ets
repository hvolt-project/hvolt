@Component
struct ChildLink {
  @Link value: number;

  build() {
    Column() {
      Text(`${this.value}`)
    }
  }
}

@Entry
@Component
struct Batch11Probe {
  plain: number = 1;

  build() {
    Column() {
      ChildLink({ value: this.plain })
    }
  }
}
