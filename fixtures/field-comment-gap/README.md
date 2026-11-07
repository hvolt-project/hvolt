# field-comment-gap

探针：字段注释跟随哪个 token。

## 读数（2026-11-07）

五个字段交替，参考侧把「下一条字段的注释」放在当前字段的**初值表达式**被复用的那一次：

| 当前字段 | 下一条注释落点（golden） |
|---|---|
| `@State a: number = 1` | 构造语句 `new ObservedPropertySimplePU(1` 之后、`, this, "a")` 之前（第 20 行） |
| `@Prop b: string = '2'` | `setInitiallyProvidedValue` 里 `this.__b.set('2'` 之后、`)` 之前（第 37 行） |
| `private c: string = '3'` | 构造语句与 setter 都没有，只在类体成员前（第 85 行） |

## 已知的排版差异（不修）

`@Prop` 的下一位注释，文本位置上应落在 `setInitiallyProvidedValue` 的 setter 内，我们落在构造语句的
`params.<name>` 之后。原因是我们的 `emit_v1_field_init` 统一把 gap 追加在构造语句里，而参考侧跟随
「字面量初值 `''`」——那个字面量只在 setter 里复用。

`compare.js` 只比较注释的**有序数组**、不比较文本位置，所以本 fixture 判 **PASS**。修它要动
`process_component_member.rs` 的两处分支（`emit_v1_field_init` 的 gap 与 `emit_v1_initial_params` 的
setter），收益仅为将来做文本级比对时一致，成本是一轮回归——因此登记为已知差异，不修。
