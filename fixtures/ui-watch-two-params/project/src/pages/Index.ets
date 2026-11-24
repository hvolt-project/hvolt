
@Entry
@Component
struct BatchProbe81b {
  @State @Watch("onChange") count: number = 0;

  onChange(first: string, second: string): void {
  }

  build() {
    Column() {
      Text(`${this.count}`)
    }
  }
}
