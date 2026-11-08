import { CommonVpnService } from '../common/CommonVpnService'

@Entry
@Component
struct CommonVpnServicePage {
  @State note: string = ''

  aboutToAppear(): void {
    this.note = typeof CommonVpnService === 'function' ? 'loaded' : 'missing'
  }

  build() {
    Column() {
      Text(this.note)
    }
  }
}
