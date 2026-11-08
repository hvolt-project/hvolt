export const SCALE: number = 2
export const OFFSET: number = 1

@Entry
@Component
struct Helper {
  build() {
    Text(`${SCALE} ${OFFSET}`)
  }
}
