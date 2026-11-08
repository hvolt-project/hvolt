import deviceInfo from '@ohos.deviceInfo'

// A local class with a method named `apiAvailable`. The reference implementation decides the
// rewrite with the type checker: the callee's type must come from `@ohos.deviceInfo.d.ts` and be
// named `apiAvailable`. This one is not, so the call must stay as written.
class Probe {
  apiAvailable(n: number): boolean {
    return n > 0
  }
}

@Entry
@Component
struct ApiAvailableShadow {
  private probe: Probe = new Probe()

  build() {
    Column() {
      if (deviceInfo.apiAvailable(11)) {
        Text('device')
      }
      if (this.probe.apiAvailable(11)) {
        Text('local')
      }
    }
  }
}
