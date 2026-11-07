# shorthand-property

对象字面量里的「键与值同名」形态：参考侧是否把 `{ name: name }` 缩写成 `{ name }`，或者反过来把
`{ name }` 展开成 `{ name: name }`。

## golden 形态

九种形态全部**保留源码写法**，参考侧不做缩写也不做展开：

| 形态 | 源码 | golden |
| --- | --- | --- |
| 键与值同名 | `{ name: name, key: key, value: value }` | 原样（L93） |
| 键与值不同名 | `{ name: key, key: name, value: value }` | 原样（L95） |
| 值是成员访问 | `{ name: this.name, key: this.ident, value: value }` | 原样（L97） |
| 值是调用 | `{ name: getFn(), key: getFn(), value: value }` | 原样（L99） |
| 嵌套对象字面量 | `{ inner: { name: name, key: key, value: value } }` | 原样（L101） |
| 箭头函数返回对象 | `(): Pair => ({ name: name, … })` | 原样（L103） |
| 数组元素是对象字面量 | `[{ name: name, … }]` | 原样（L105） |
| 子组件调用参数对象 | `ChildRow({ name: this.name, tag: this.ident })` | 原样（L211） |
| `@Reusable` 子组件 | `ReusableRow({ name: this.name })` | 原样（`observeRecycleComponentCreation`，L232） |

`all.sh shorthand-property` 一次通过，说明 hvolt 现在对这九种形态与参考一致。

## 这个名字从哪来

这一条对应 `differing` 列表里 `PropertyAssignment ↔ ShorthandPropertyAssignment` 那一簇（5 个文件）。
本 fixture 无法复现该簇的差异，所以它记录的是「这些形态不该被改写」这一事实，而不是一个待修的缺陷。

## 复现

```sh
scripts/oracle/new-fixture.sh shorthand-property
HOME=/tmp/hvigor-home scripts/oracle/run-hvigor.sh shorthand-property
scripts/oracle/all.sh shorthand-property     # PASS
```
