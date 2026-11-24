
import { run } from "../lib/helper";

function callIt(): number {
  return run();
}

@Entry
@Component
struct BatchProbe64 {
  build() {
    Column() {
      Text("x")
    }
  }
}
