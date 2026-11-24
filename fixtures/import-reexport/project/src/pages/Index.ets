
import { run } from "../lib/reexport";

function callIt(): number {
  return run();
}

@Entry
@Component
struct BatchProbe84 {
  build() {
    Column() {
      Text("x")
    }
  }
}
