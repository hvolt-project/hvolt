@Entry
@Component
struct CarryGate {
  // PA
  @Consume a: boolean
  /**
   * DOC_A
   */
  @State b: number = 0

  // PB
  @Consume c: boolean
  /**
   * DOC_B
   */
  d: number = 1

  // PC
  @Consume e: boolean
  /**
   * DOC_C
   */
  public static f: number = 2

  // PD
  @Consume g: boolean
  /**
   * DOC_D
   */
  h(): number {
    return 3
  }

  build() {
    Column() {
      Text(`${this.b}${this.d}${CarryGate.f}${this.h()}`)
    }
  }
}
