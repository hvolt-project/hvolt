
import { run } from "../lib/helper.ts";

function callIt(): number {
  return run();
}

@Entry
@Component
struct BatchProbe58 {
  build() {
    Column() {
      Text("x")
    }
  }
}
