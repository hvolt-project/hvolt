
import { default as helper } from "../lib/helper";

function callIt(): number {
  return 1;
}

@Entry
@Component
struct BatchProbe52 {
  build() {
    Column() {
      Text("x")
    }
  }
}
