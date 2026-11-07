// The `@LocalBuilder` file flag: the header `if (PUV2ViewBase.contextStack === undefined)`,
// the context-stack bracket in every `initialRender` / `rerender`, and the method itself as a
// property arrow whose body starts from the parent the stack holds.
@Component
struct LbChip {
  @Prop label: string = ''

  build() {
    Text(this.label)
      .fontSize(12)
  }
}

@ComponentV2
struct LbPanel {
  @Local count: number = 0

  @LocalBuilder
  private title(g: string) {
    ForEach([g], (item: string) => {
      Text(item)
        .fontSize(16)
    })
  }

  @LocalBuilder
  content() {
    LbChip({ label: 'x' })
  }

  @Builder
  footer(t: string) {
    Text(t)
      .fontSize(10)
  }

  build() {
    Column() {
      this.title('hello')
      this.content()
      this.footer('bye')
    }
  }
}

@Entry
@Component
struct Index {
  @State shown: boolean = true

  build() {
    Column() {
      LbPanel()
      Text(this.shown ? 'on' : 'off')
    }
  }
}
