function needString(): string {
  return 42;
}

@Entry
@Component
struct AssignProbe {
  @State label: string = 'a';
  numbers: number[] = [1, 'two'];

  build() {
    Column() {
      Button('x')
        .onClick(() => {
          this.label = 1;
        })
      Text(`${needString()}${this.numbers.length}`)
    }
  }
}
