// Regression cover for three spelling-based shortcuts:
//  1. `export struct` declarations must be recognised as structs;
//  2. a builder method may be lowercase and take parameters;
//  3. a comment inside a child's argument list must not swallow the generated line.
function isHex(hex: string): boolean {
  return /^[0-9A-F]{2}$/.test(hex);
}

@Entry
@Component
export struct LocalBuilderView {
  @State count: number = 0

  @Builder
  dialogContent(data: string) {
    ForEach([data], (item: string) => {
      Text(`${item}`)
    })
  }

  build() {
    Column() {
      Text(isHex('FF') ? 'ok' : 'bad')
      this.dialogContent(`${this.count}`)
    }
  }
}
