import { PlainUnit } from '../util/PlainUnit'

@Entry
@Component
struct IndentUnit {
  /**
   * FIELD_BLOCK
   */
  private fieldProbe: number = 0

  /**
   * TWO_SPACE_BLOCK
   */
  private twoSpace(): number {
    /**
     * TWO_SPACE_INNER
     */
    return 1
  }

    /**
     * FOUR_SPACE_BLOCK
     */
  private fourSpace(): number {
    return 2
  }

  private templateText(): string {
    return `line1
      TEMPLATE_INDENT
    line3`
  }

  build() {
    Column() {
      Text(`${this.twoSpace()}${this.fourSpace()}`)
      Text(this.templateText()) // TRAILING_STMT
      Text(`${this.fieldProbe}${PlainUnit.value()}`)
    }
  }
}
