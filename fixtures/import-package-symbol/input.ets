import { helper, nope } from 'mylib';

@Entry
@Component
struct PackageImportProbe {
  build() {
    Column() {
      Text(helper())
    }
  }
}
