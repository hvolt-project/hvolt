
import { cyclicA } from "../lib/circa";

function callIt(): number {
  return cyclicA();
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
