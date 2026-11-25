class Plain {
  size: number = 1;
}

@Component
struct Child {
  @ObjectLink model: Plain;

  build() {
    Column() {
      Text(`${this.model.size}`)
    }
  }
}

@Entry
@Component
struct Batch10Probe {
  plain: Plain = new Plain();

  build() {
    Column() {
      Child({ model: this.plain })
    }
  }
}
