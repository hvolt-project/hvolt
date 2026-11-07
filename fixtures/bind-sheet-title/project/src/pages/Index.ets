// Where the identifier `BindSheetTitleCustomBuilder` comes from: the sheet options' `title`
// slot takes a CustomBuilder, and the reference may hoist each written shape into a named
// variable. Each numbered block is one candidate shape; the golden decides.

@Entry
@Component
struct BindSheetTitleProbe {
  @State shown: boolean = false

  @Builder
  sheetBody() {
    Text('body')
  }

  @Builder
  sheetTitle() {
    Text('title')
  }

  @Builder
  BindSheetTitleCustomBuilder() {
    Text('named title')
  }

  build() {
    Column() {
      // 1 title as an inline arrow
      Column()
        .bindSheet($$this.shown, this.sheetBody, {
          title: () => {
            Text('inline title')
          }
        })
      // 2 title as a builder reference
      Column()
        .bindSheet($$this.shown, this.sheetBody, { title: this.sheetTitle })
      // 3 a builder method literally named BindSheetTitleCustomBuilder
      Column()
        .bindSheet($$this.shown, this.sheetBody, { title: this.BindSheetTitleCustomBuilder })
    }
  }
}
