import { AppStorageV2, PersistenceV2 } from "@kit.ArkUI"
import { TypeConstructorWithArgs } from "@ohos.arkui.StateManagement"

export function getPersistence<T extends object>(type: TypeConstructorWithArgs<T>, factory: () => T): T | undefined {
  return PersistenceV2.connect(type, type.name, factory)
}

export function getAppStore<T extends object>(type: TypeConstructorWithArgs<T>, factory: () => T): T | undefined {
  return AppStorageV2.connect(type, type.name, factory)
}
