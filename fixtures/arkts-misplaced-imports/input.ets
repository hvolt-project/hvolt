import "../lib/side";

export { run as renamed } from "../lib/helper";

export default function defaultValue(): number {
  return 1;
}

import { run } from "../lib/helper";

@Entry
@Component
struct BatchProbe {
  build() {
    Column() {
      Text(`${run()}`)
    }
  }
}
