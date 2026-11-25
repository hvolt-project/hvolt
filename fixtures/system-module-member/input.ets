import util from '@ohos.util';

@Entry
@Component
struct SinceLatestProbe {
  build() {
    Column() {
      Text(String(util.AutoFinalizer === undefined))
    }
  }
}
