import {
  // how much to scale by
  SCALE,
  // how much to scale by
  OFFSET,
} from './Helper'

@Entry
@Component
struct ImportComment {
  @State value: number = 0

  build() {
    Column() {
      Text(`${this.value * SCALE + OFFSET}`)
        .fontSize(16)
    }
  }
}
