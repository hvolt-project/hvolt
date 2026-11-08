import { pageBuilder } from './Shared';

@Entry
@Component
struct ImportHost {
  build() {
    Column() {
      pageBuilder('shared')
    }
  }
}
