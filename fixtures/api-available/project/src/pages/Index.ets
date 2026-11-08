import deviceInfo from '@ohos.deviceInfo'

@Entry
@Component
struct ApiAvailableView {
  build() {
    Column() {
      if (deviceInfo.apiAvailable(11)) {
        Text('supported')
      } else {
        Text('unsupported')
      }
      if (deviceInfo.apiAvailable('5.0.5(17)')) {
        Text('point')
      }
    }
  }
}
