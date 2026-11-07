@Builder
function FallbackHeader() {
  Text('')
}

@Component
struct ChildBox {
  @BuilderParam header: () => void = FallbackHeader
  label: string = ''

  build() {
    Column() {
      this.header()
      Text(this.label)
    }
  }
}

interface RowData {
  name: string
}

@Entry
@Component
struct PropertyAccessThis {
  @State flag: boolean = true
  @State primary: string = '#ff0000'
  @State secondary: string = '#00ff00'
  @State rows: RowData[] = [{ name: 'a' }, { name: 'b' }]

  color(): string {
    return this.flag ? this.primary : this.secondary
  }

  handler(): void {
    this.flag = !this.flag
  }

  work(): void {
    this.flag = true
  }

  @Builder
  headerB(text: string) {
    Text(text).fontSize(12)
  }

  @Builder
  footerB() {
    Text('footer')
  }

  @Builder
  rowFooter(text: string) {
    Text(text).fontSize(10)
  }

  @Builder
  refreshB() {
    Text('refreshing')
  }

  build() {
    Column() {
      Text('a').fontColor(this.color())
      Text('b').onClick(this.handler)
      Text('c').onClick(() => {
        this.work()
      })
      Text('d').onClick(this.work)
      Text('e').fontColor(this.flag ? this.primary : this.secondary)
      Text('f').onClick(this.work.bind(this))
      ChildBox({ header: this.footerB, label: 'x' })
      List() {
        ForEach(this.rows, (row: RowData) => {
          ListItemGroup({ header: this.headerB(row.name), footer: this.rowFooter(row.name) }) {
            ListItem() {
              Text(row.name)
            }
          }
        })
      }
      Refresh({ refreshing: this.flag, builder: this.refreshB() }) {
        Text('body')
      }
      this.footerB()
    }
  }
}
