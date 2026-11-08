@ObservedV2
class Model {
  @Trace name: string = ''
}

@Entry
@ComponentV2
struct V2View {
  @Local count: number = 0
  @Provider('theme') theme: string = 'light'

  @Computed get doubled(): number {
    return this.count * 2
  }
  @Monitor('count') onCountChanged(mon: IMonitor): void {
  }

  build() {
    Column() {
      V2Card({ title: 'x', once: 1 })
      V1Badge({ label: this.theme })
    }
  }
}

@ComponentV2
struct V2Card {
  @Param title: string = ''
  @Param @Once once: number = 0
  @Event onTap: () => void = () => {}
  @Consumer('theme') consumedTheme: string = ''

  build() {
    Text(this.title)
  }
}

@Component
struct V1Badge {
  @Prop label: string = ''

  build() {
    Text(this.label)
  }
}