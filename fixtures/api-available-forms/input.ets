// `process_available_statement.js` rewrites the callee through `transformExpression`, so the
// access form survives: `deviceInfo?.apiAvailable(11)` keeps its chain. The visitor is
// `ts.visitEachChild`, which also descends into a template literal's `${…}`, so the call in the
// `${}` below is rewritten as well. A version string does not need the `(api)` suffix: the
// reference's own pattern makes it optional (`(\d+\.\d+\.\d+)(\(\d+\))?`).
//
// The element form (`deviceInfo['apiAvailable'](11)`) is supported by that same function but
// cannot be written in ArkTS at all — the checker rejects the namespace access with
// `10605114 Namespaces cannot be used as objects (arkts-no-ns-as-obj)` — so it is not exercised
// here.
import deviceInfo from '@ohos.deviceInfo';

@Entry
@Component
struct ApiAvailableForms {
  build() {
    Column() {
      if (deviceInfo.apiAvailable(11)) {
        Text('dot')
      }
      if (deviceInfo?.apiAvailable(11)) {
        Text('chain')
      }
      if (deviceInfo.apiAvailable('5.0.5')) {
        Text('no-suffix')
      }
      if (deviceInfo.apiAvailable('5.0.5(17)')) {
        Text('suffix')
      }
      Text(`${deviceInfo.apiAvailable(12)}`)
    }
  }
}
