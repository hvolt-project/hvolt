function tagged(strings: TemplateStringsArray, value: number): string {
  return `${value}`;
}

const composed = tagged`value ${1}`;

@Entry
@Component
struct TaggedProbe {
  build() {
    Column() {
      Text(composed)
    }
  }
}
