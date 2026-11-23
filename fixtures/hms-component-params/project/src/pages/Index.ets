import { MapComponent } from '@hms.core.map.MapComponent';

@Entry
@Component
struct HmsComponentProbe {
  build() {
    Column() {
      MapComponent({ nope: 1 })
    }
  }
}
