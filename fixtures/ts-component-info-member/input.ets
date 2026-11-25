
import { componentUtils } from "@kit.ArkUI";

function read(): number {
  const rect = componentUtils.getRectangleById("id");
  return rect.width;
}

@Entry
@Component
struct BatchProbe57 {
  build() {
    Column() {
      Text("x")
    }
  }
}
