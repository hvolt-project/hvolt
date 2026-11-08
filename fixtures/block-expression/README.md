# block-expression

花括号的取舍：`if` 分支、`else` 分支、`ForEach` 回调、箭头函数体、方法体、`@Builder` 方法体
只有一条语句时，参考侧是否省略块、是否把单条语句提成表达式语句。

## golden 形态

六种形态全部**保留源码的块形状**，参考侧不省花括号，也不把块体改成表达式：

| 形态 | 源码 | golden |
| --- | --- | --- |
| `if` 分支单条语句 | `if (flag) { Text('one') }` | `If.create()` + `if (this.flag) { this.ifElseBranchUpdateFunction(0, () => { … }) }`，带花括号（L78） |
| `if` / `else` 两臂体量不同 | 一臂一条、一臂两条 | 两臂都带花括号，各自的 `ifElseBranchUpdateFunction` 体内是完整块（L94、L102） |
| `ForEach` 回调体一条语句 | `ForEach(items, (item: string) => { Text(item) })` | 回调体保持块（L117–L126） |
| 箭头函数块体 | `(n) => { return n + 1 }` | 原样保留块体（L61） |
| 箭头函数表达式体 | `(n) => n * 2` | 原样保留表达式体（L60） |
| 方法体一条语句 | `oneLine() { return 1 }` | 原样保留块体（L54） |
| `@Builder` 方法体一条语句 | `singleBuilder() { Text('single') }` | `singleBuilder(parent = null) { … }` 带块体（L66），调用处 `this.singleBuilder.bind(this)()`（L139） |

`all.sh block-expression` 一次通过，说明 hvolt 现在对这六种形态与参考一致。

## 这个名字从哪来

这一条对应 `differing` 列表里 `Block ↔ ExpressionStatement` 那一簇（3 个文件：`QRCodeComponent.ts`、
`SubmitApp.ts`、`Xb_SettingItem.ts`）。本 fixture 无法复现该簇的差异，因此它记录的是「这些形态不该被
改写」这一事实，而不是一个待修的缺陷。

## 复现

```sh
scripts/oracle/new-fixture.sh block-expression
HOME=/tmp/hvigor-home scripts/oracle/run-hvigor.sh block-expression
scripts/oracle/all.sh block-expression     # PASS
```
