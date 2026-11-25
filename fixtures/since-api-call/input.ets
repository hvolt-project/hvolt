import util from '@ohos.util';

@Entry
@Component
struct SinceProbe {
  build() {
    Column() {
      Text(util.getMainThreadStackTrace())
    }
  }
}
