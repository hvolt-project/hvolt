
import { outer } from "../lib/inner";

function callIt(): number {
  return outer();
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
