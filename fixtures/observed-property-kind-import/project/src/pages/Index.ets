import { ImportedKind, ImportedAlias } from '../common/Kinds'

// 状态字段的类型来自别的文件：参考侧靠类型检查器解析，hvolt 目前拿不到（已知缺口）。
@Entry
@Component
struct ObservedPropertyKindImport {
  @State importedEnum: ImportedKind = ImportedKind.A
  @State importedAlias: ImportedAlias = 'x'
  @State importedAliasUnion: ImportedAlias | undefined = undefined

  build() {
    Column() {
      Text(`${this.importedEnum}${this.importedAlias}`)
    }
  }
}
