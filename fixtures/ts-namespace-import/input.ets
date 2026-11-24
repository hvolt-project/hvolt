
import { run } from "../lib/wrapper";

const value = run();

@Entry
@Component
struct BatchProbe8 {
  build() {
    Column() {
      Text("x")
    }
  }
}
