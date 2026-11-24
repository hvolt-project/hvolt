import { run } from "../lib/helper" assert { type: "json" };

@Entry
@Component
struct AssertProbe {
  build() {
    Column() {
      Text(`${run()}`)
    }
  }
}
