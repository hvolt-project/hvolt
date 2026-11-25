import { helper } from "shared";

@Entry
@Component
struct SharedImportProbe {
  build() {
    Column() {
      Text(helper())
    }
  }
}
