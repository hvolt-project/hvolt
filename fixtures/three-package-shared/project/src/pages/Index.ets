import { sharedValue } from 'shared';

@Entry
@Component
struct MultiPackageProbe {
  build() {
    Column() {
      Text(`${sharedValue()}`)
    }
  }
}
