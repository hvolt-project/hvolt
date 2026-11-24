export { dup } from '../lib/c';
export { dup } from '../lib/d';

@Entry
@Component
struct DuplicateExportProbe {
  build() {
    Column() {
      Text('x')
    }
  }
}
