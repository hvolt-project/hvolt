@Entry
@ComponentV2
struct V2ResetOrder {
  @Local before: number = 1;

  @Computed
  get doubled(): number {
    return this.before * 2;
  }

  @Local after: number = 2;

  @Monitor('before')
  onBefore(mon: IMonitor): void {
  }

  build() {
    Column() {
      Text(`${this.doubled}${this.after}`)
    }
  }
}
