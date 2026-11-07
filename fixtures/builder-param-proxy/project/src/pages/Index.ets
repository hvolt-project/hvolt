// Which argument shapes the reference rewrites when a builder is called, and which it forwards
// as written. The first probe wrote object literals into a `@BuilderParam` field, which the
// ArkTS checker rejects (`Type 'Cfg' provides no match for the signature '(): void'`), so each
// shape sits at a call site whose declared parameter type accepts it.

class Cfg {
  a: number = 1
  b: Cfg | null = null
  run: () => void = () => {
  }
}

const SHARED_CFG: Cfg = { a: 1, b: null, run: () => {
} }

@Builder
function globalContent(cfg: Cfg) {
  Column() {
    Text(`${cfg.a}`)
  }
}

class Cfg2 {
  title: string = ''
}

@Builder
function globalTitleObj(cfg: Cfg2) {
  Text(cfg.title)
}

@Builder
function globalTitle(text: string) {
  Text(text)
}

@Builder
function globalCount(n: number) {
  Text(`${n}`)
}

@Entry
@Component
struct BuilderParamProxyProbe {
  @State shown: boolean = false
  @State pageTitle: string = 'probe'
  private shared: Cfg = SHARED_CFG

  @Builder
  memberContent(cfg: Cfg) {
    Text(`${cfg.a}`)
  }

  @Builder
  memberSheet() {
    Text('sheet')
  }

  @Builder
  sheetTitle() {
    Text('title')
  }

  build() {
    Column() {
      // 1 a lone object literal
      globalContent({ a: 1, b: null, run: () => {
      } })
      // 2 an object literal carrying a nested object and an arrow
      globalContent({ a: 1, b: { a: 2, b: null, run: () => {
      } }, run: () => {
      } })
      // 3 a declared object constant
      globalContent(SHARED_CFG)
      // 4 a reference rather than a call
      globalContent(this.shared)
      // 5 a member @Builder call with an object literal
      this.memberContent({ a: 4, b: null, run: () => {
      } })
      // 6 a literal argument (control)
      globalCount(2)
      // 7 a @State string read as a builder argument
      globalTitle(this.pageTitle)
      // 8 a @State string read inside the proxy object
      globalTitleObj({ title: this.pageTitle })
      // a CustomBuilder argument that is a reference (control)
      Column()
        .bindSheet($$this.shown, this.memberSheet)
      // an inline arrow where a CustomBuilder is expected
      Column()
        .bindSheet($$this.shown, () => {
          this.memberSheet()
        })
      // a builder used as the sheet title
      Column()
        .bindSheet($$this.shown, this.memberSheet, { title: this.sheetTitle })
    }
  }
}
