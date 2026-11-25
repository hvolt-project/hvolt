import { helper } from '../lib/barrel';

@Entry
@Component
struct ExportStarProbe {
  build() {
    Column() {
      Text(helper())
    }
  }
}
