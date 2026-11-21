@Entry
@Component
struct TypeStructMember {
  counter: number = 1;

  bump() {
    this.counter += 1;
  }

  build() {
    Column() {
      Text(`${this.counter}${this.notAField}`)
    }
  }
}