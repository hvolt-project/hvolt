@Entry
@Component
struct BlockExpression {
  @State flag: boolean = true
  @State items: string[] = ['a', 'b']

  oneLine(): number {
    return 1
  }

  onlyStatement(): void {
    this.flag = !this.flag
  }

  compute(): number {
    const double = (n: number): number => n * 2
    const plusOne = (n: number): number => {
      return n + 1
    }
    return double(plusOne(1))
  }

  @Builder
  singleBuilder() {
    Text('single')
  }

  build() {
    Column() {
      if (this.flag) {
        Text('one')
      }
      if (this.flag) {
        Text('a')
      } else {
        Text('b')
        Text('c')
      }
      ForEach(this.items, (item: string) => {
        Text(item)
      })
      Button('x').onClick(() => {
        this.onlyStatement()
      })
      Text(`${this.oneLine()}${this.compute()}`)
      this.singleBuilder()
    }
  }
}
