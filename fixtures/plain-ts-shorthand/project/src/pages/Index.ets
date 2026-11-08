import { makeBag } from '../util/Pair'

@Entry
@Component
struct PlainTsShorthandPage {
  @State note: string = ''

  aboutToAppear(): void {
    this.note = makeBag().destination
  }

  build() {
    Column() {
      Text(this.note)
    }
  }
}
