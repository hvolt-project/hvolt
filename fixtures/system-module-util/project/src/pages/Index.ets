
import { util } from "@kit.ArkTS";

function read(): string {
  return util.missingFormat("%d", 1);
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
