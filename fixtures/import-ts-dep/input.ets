
import { plainFn } from "../lib/plain";

function callIt(): number {
  return plainFn();
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
