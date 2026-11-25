export { default } from "../lib/helper";

@Entry
@Component
struct DefaultReexportProbe {
  build() {
    Column() {
      Text("x")
    }
  }
}
