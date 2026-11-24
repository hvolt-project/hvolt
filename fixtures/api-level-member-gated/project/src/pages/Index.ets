import window from "@ohos.window";

@Entry
@Component
struct ApiLevelProbe {
  build() {
    Column() {
      Text(`${window.getHostContext() !== undefined}`)
    }
  }
}
