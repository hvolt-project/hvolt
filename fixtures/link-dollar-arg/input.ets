@Entry
@Component
struct DollarProbe {
  @State count: number = 1;

  build() {
    Column() {
      DollarChild({ total: this.$count })
      DollarChild({ total: $count })
    }
  }
}

@Component
struct DollarChild {
  @Link total: number;

  build() {
    Text(`${this.total}`)
  }
}
