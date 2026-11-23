import { helper } from '../lib/barrel';

@Entry
@Component
struct ExportStarEtsProbe {
  build() {
    Column() {
      Text(helper())
    }
  }
}
