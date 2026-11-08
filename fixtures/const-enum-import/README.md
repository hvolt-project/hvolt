# const-enum-import

`const enum` 的成员跨文件引用：参考实现把它内联成字面量，并把 import 收成 `import type`。
这张表只能由**项目级**收集提供 —— 引用点所在的文件里没有成员值。

## 参考的发射

`input.ets`：`import { ImportedKind } from '../common/Shared'`，`Shared.ets` 里
`export const enum ImportedKind { One = 1, Two = 2 }`。

`golden/Index.ts`：

```ts
import type { ImportedKind } from '../common/Shared';   // L6：值被内联后 import 收成 type
...
Text.layoutWeight(1);                                    // L32
Text.fontSize(2);                                        // L33
```

值被内联、且 import 变成 `import type`（请求路径保持源码写法，不解析成 `@normalized:`）。

## 实现

两头各一处，都不在逐文件的 pass 里：

1. **收集**：`src/build/ets_assemble.rs` 在降级任何文件之前，把每个模块的源交给
   `enum_init::collect_const_enums`，得到 `name → [(member, literal)]`，再
   `set_const_enums` 发布（thread-local，理由与 `@AnimatableExtend` 表相同：构建一次只降级一个文件）。
   **冲突策略**：同名但成员值不同 → 该名字整张表丢弃（调用点无从判断指的是哪一个，
   错的内联比不内联危险）；同名且值相同 → 保留一份；普通 `enum` 不进表。
2. **使用**：`fill_enum_initializers` 把项目表和本文件的表合并，**本文件声明优先**
   （模块作用域才是调用点看到的东西）；值位判定用 oxc AST，见 `const-enum-inline`。
3. **import 形态**：`ets_assemble::declares_as_type` 增加一条 —— 目标模块把该名字声明为
   `const enum` 时按类型导入处理（成员已在调用点被抹掉，运行时没有绑定可读），于是
   `import type` + 请求保持源码写法。

## 复现

```sh
scripts/oracle/new-fixture.sh const-enum-import
HOME=/tmp/hvigor-home scripts/oracle/run-hvigor.sh const-enum-import
scripts/oracle/all.sh const-enum-import     # PASS
```

注意：`new-fixture.sh` 只拷 `input.ets`，`project/src/common/Shared.ets` 是手工放的；
重跑 `new-fixture.sh` 不会删它（rsync 不删除多余文件）。同文件的形态在
`fixtures/const-enum-inline`。