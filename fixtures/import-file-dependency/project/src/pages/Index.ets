import { helper } from 'lib';

@Entry
@Component
struct LibImportProbe {
  build() {
    Column() {
      Text(helper())
    }
  }
}
