
import { anything } from "@ohos.nonexistentmodule";

function callIt(): number {
  return 1;
}

@Entry
@Component
struct BatchProbe54 {
  build() {
    Column() {
      Text("x")
    }
  }
}
