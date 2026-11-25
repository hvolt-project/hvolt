
import { AutoFinalizer, util } from "@kit.ArkTS";

const made = util.AutoFinalizer;
const missing = util.NotThere;

@Entry
@Component
struct BatchProbe20 {
  build() {
    Column() {
      Text("x")
    }
  }
}
