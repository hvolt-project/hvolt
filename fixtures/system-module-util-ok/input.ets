
import { util } from "@kit.ArkTS";

function read(): string {
  return util.format("%d", 1);
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
