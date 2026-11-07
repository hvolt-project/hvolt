import { AppStorageV2, PersistenceV2 } from "@kit.ArkUI"
import { TypeConstructorWithArgs } from '@ohos.arkui.StateManagement';
import { getPersistence } from './Plain';

class ProbeStore {
  value: number = 1;
}

export function restore<T extends object>(type: TypeConstructorWithArgs<T>, factory: () => T): T | undefined {
  return PersistenceV2.connect(type, type.name, factory);
}

@Entry
@Component
struct KitTwoImports {
  @State label: string = 'two'

  build() {
    Column() {
      Text(this.label + String(getPersistence(ProbeStore, () => new ProbeStore()) !== undefined))
    }
  }
}
