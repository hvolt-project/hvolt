
import { window } from "@kit.ArkUI";

function read(context: object): void {
  window.missingApi(context);
}

@Entry
@Component
struct BatchProbe61 {
  build() {
    Column() {
      Text("x")
    }
  }
}
