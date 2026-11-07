import { AppStorageV2, PersistenceV2 } from '@kit.ArkUI';

class ProbeStore {
  value: number = 1;
}

const connected: ProbeStore = AppStorageV2.connect(ProbeStore, 'probe', () => new ProbeStore())!;

@Entry
@Component
struct KitArkuiState {
  @State label: string = 'state'

  build() {
    Column() {
      Text(this.label + String(connected.value))
    }
  }
}
