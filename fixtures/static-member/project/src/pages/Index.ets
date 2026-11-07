@Entry
@Component
struct StaticProbe {
  static readonly TAG: string = 'probe';
  @State count: number = 0;

  static describe(): string {
    return StaticProbe.TAG;
  }

  build() {
    Column() {
      Text(`${this.count}`)
    }
  }
}
