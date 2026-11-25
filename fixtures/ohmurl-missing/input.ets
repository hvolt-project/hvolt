
import { run } from "@bundle:com.example.batch/entry/lib/missing";

function callIt(): number {
  return 1;
}

@Entry
@Component
struct BatchProbe54 {
  build() {
    Column() {
      Text("x")
    }
  }
}
