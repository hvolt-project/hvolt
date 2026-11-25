interface Options {
  size: number;
}

@Entry
@Component
struct Batch4Probe {
  @State count: number = 0;
  @State options: Options = { size: 1 };

  @Watch("onCount")
  onCountChanged() {
    this.count = this.count + 1;
  }

  build() {
    Column() {
      Text(`${this.count}`)
        .stateStyles({ normal: this.normalStyle })
    }
  }

  @Styles normalStyle() {
    .width(10)
  }
}
