@Builder
function globalBuilder() {
  Text('gb')
}

@Component
struct TrailingProbe {
  @BuilderParam slot: () => void = globalBuilder; // TRAILING_FIELD
  @State private count: number = 0; // TRAILING_PLAIN
  @BuilderParam tail: () => void = globalBuilder; // TRAILING_THEN_LEADING
  // LEADING_AFTER_TRAILING
  onUpdate(): void {
    this.count = 1; // TRAILING_STMT
  }

  build() {
    Column() {
      this.slot()
      this.tail()
      Text(`${this.count}`)
    }
  }
}

@Component
struct SingleFieldProbe {
  @State private only: number = 0; // ONLY_FIELD

  build() {
    Text(`${this.only}`)
  }
}

@Component
struct PairProbe {
  @BuilderParam slotA: () => void = globalBuilder; // TRAIL_A
  // LEADING_A
  onUpdateA(): void {
    this.slotA()
  }
  @BuilderParam slotB: () => void = globalBuilder; // TRAIL_B
  // LEADING_B
  onUpdateB(): void {
    this.slotB()
  }

  build() {
    Column() {
      this.slotA()
      this.slotB()
    }
  }
}

@Entry
@Component
struct TrailingIndex {
  build() {
    Column() {
      TrailingProbe({ tail: globalBuilder })
      SingleFieldProbe()
      PairProbe({ slotB: globalBuilder })
    }
  }
}

class ProbeOptions {
  value: number = 0
}

class ProbeInfo {
  flag: boolean = true
}

// A shape copied from the mccharts component: a field carrying @Watch, other state fields,
// then a @BuilderParam whose line ends in a comment, then a comment above a plain method that
// has a parameter, a return type and a real body.
@Component
export struct McLikeProbe {
  @Prop @Watch('onOptionsChanged') options: ProbeOptions;
  @State chartOption: number = 0
  private callback: Function = () => {}
  private click: Function = (event: ProbeInfo) => {}

  @BuilderParam customTooltip: (info: ProbeInfo) => void = globalBuilder; // PROBE_TRAILING
  // PROBE_LEADING
  onOptionsChanged(options: ProbeOptions): void {
    this.chartOption = options.value
    this.callback = () => {}
  }

  // @Watch 回调 AT_WATCH_IN_COMMENT
  aboutToAppear() {
    this.chartOption = 1
  }

  build() {
    Column() {
      this.customTooltip({ flag: true })
    }
  }
}
