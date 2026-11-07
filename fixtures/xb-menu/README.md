# xb-menu

整份复刻 `xb_components/components/Xb_Menu.ets`（36 行，2 条 import）。它是 59 个 `differing`
文件里最小的一个。

## 复刻方式

- 正文逐字节取自 `clashbox-flat/xb_components/components/Xb_Menu.ets`，只在末尾追加一个
  `@Entry` 页面来调用这个全局 `@Builder function`；
- import 换桩：`../settings/SettingsCommonOp` 的 `MenuItem` 接口用同目录最小桩
  （`project/src/settings/SettingsCommonOp.ets`），`@kit.ArkUI` 的 `SymbolGlyphModifier` 保留；
- 没有补资源：`$r('sys.symbol.checkmark')` 是系统资源。

## 差异

AST 级只有一条：`ForEach` 的更新调用接收者。参考侧在全局 `@Builder function` 里发射
`(parent ? parent : this).forEachUpdateFunction(…)`，hvolt 发射 `this.forEachUpdateFunction(…)`。
其余 hunk 都是排版（缩进、括号、空方法体、对象字面量布局、`registerNamedRoute` 换行）。

最小形态见 `fixtures/builder-for-each`（16 行）。
