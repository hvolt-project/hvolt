import { callIt } from "../lib/wrapper";

@Entry
@Component
struct TsImportEtsProbe {
  build() {
    Column() {
      Text(`${callIt()}`)
    }
  }
}
