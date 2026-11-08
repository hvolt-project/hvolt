import { ImportedKind } from '../common/Shared'

// `const enum` 的成员跨文件引用时，参考实现同样内联成字面量（并把 import 收成 `import type`）。
// 值在另一个文件的文本里，`lower/enum_init.rs` 这个逐文件的 pass 看不到它的成员 —— 已知缺口。
@Entry
@Component
struct ConstEnumImport {
  build() {
    Column() {
      Text('imported')
        .layoutWeight(ImportedKind.One)
        .fontSize(ImportedKind.Two)
    }
  }
}
