@Entry
@Component
struct ReadonlyProbe {
  readonly ro: number = 2
  private readonly pr: number = 3
  public readonly pu: number = 4
  @State readonly st: number = 5
  plain: string = 'a'
  private plainPrivate: string = 'b'

  build() {
    Column() {
      Text(`${this.ro}-${this.pr}-${this.pu}-${this.st}-${this.plain}-${this.plainPrivate}`)
    }
  }
}
