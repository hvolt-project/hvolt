@Builder
function globalBuilder() {
  Text('gb')
}

// PRO_PROBE
@Entry
@Component
struct MethodProbe {
  private off: number | null = null
  private started: boolean = false
  @State value: number = 0
  @Prop @Watch('onCountUpdated') watched: number = 0
  @Link renderType: string
  @BuilderParam customTooltip: () => void = globalBuilder; // 自定义组件

  onCountUpdated(options: number): void {
    // this.off = null
    // if (!this.off) {
    //   this.started = true
    // }
    this.value = options
  }

  aboutToAppear(): void {
    // this.started = false
    // this.value = 1
    this.value = this.value + 1
  }

  aboutToDisappear(): void {
    // this.started = false
    // this.off = null
  }

  build() {
    Column() {
      // Text('commented out')
      Text(`${this.value}`)
    }
  }
}
