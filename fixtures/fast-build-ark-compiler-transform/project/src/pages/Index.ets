// The transform hook is a pass-through for ArkTS sources: `shouldETSOrTSFileTransformToJS`
// returns false for every file that is being compiled, and ArkTS rejects `@ts-nocheck`
// (arkts-strict-typing-required), so the only reachable behaviour is "leave the source as is".
@Entry
@Component
struct PassThroughView {
  @State count: number = 0

  build() {
    Column() {
      Text(`${this.count}`).fontSize(20)
      Button('add').onClick(() => {
        this.count += 1
      })
    }
  }
}
