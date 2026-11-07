@Component
struct GapProbe {
  @State a: number = 1
  // NEXT_A
  @Prop b: string = '2'
  // NEXT_B
  private c: string = '3'
  // NEXT_C
  @State d: number = 4
  // NEXT_D
  @Prop e: string = '5'

  build() {
    Column() {
      Text(`${this.a}${this.b}${this.c}${this.d}${this.e}`)
    }
  }
}

@Entry
@Component
struct GapIndex {
  build() {
    Column() {
      GapProbe({ e: 'x' })
    }
  }
}
