# 装配层 pass 改由 AST 定位（2026-11-06）

## 为什么改

lowering 之后、写盘之前的那几个 pass 原来靠 token 流找目标：向前找 `from` 猜语句起点、逐行状态机切 import 语句、数 `builder:` 后面的四个 token。token 扫描能跳过字符串与注释，但它对"这是不是一条语句""这个绑定在哪"只能靠位置推，推错就静默改错。

现在这些判定读语法树：`ImportDeclaration` 的 specifier、`ObjectProperty` 的值、`CallExpression` 的被调、`StaticMemberExpression` 的两端。**定位来自节点，写入仍是按 span 的文本替换**：重打印整棵树会把行号全挪走，而 source map 是按 lowering 的行号生成的。

## 对照 hvigor 产物查出并修的差异

审计方式是黑盒：为每个可疑行为造一个 fixture，跑 `new-fixture.sh` → `run-hvigor.sh` 取 golden → `run-hvolt.sh` → `compare.js` + `compare-artifacts.py`。

| 现象 | hvigor 的行为 | 我们原来的行为 | 修法 |
|---|---|---|---|
| 普通 import 只是类型 | 输出 `import type { Shape } from './Types';`，specifier 保持相对 | 整条 import 删掉 | 推断出的 type-only 保留原 specifier；显式 `import type` 仍归一化（`inferred_types` 在两个 pass 之间传递） |
| `$r('app.x')` 的对象键 | `{ "id": …, "type": … }` 字符串键 | 标识符键 | 生成字符串键 |
| 只声明类型的模块 | 擦除后没有代码：无 abc record、无 source map | 照样编译出空模块 | `module_has_no_runtime_code` 判定后跳过写盘与 map |
| `Image($r(…))` 之后 | 不生成 `pop()` | 无条件 `pop()` | pop 条件改为「非 atomic 组件，或 Blank/Search」；atomic 名单取自 SDK `components/*.json` |

对应 fixture：`fixtures/import-type-audit`、`fixtures/resource-audit`（都带 golden 与 covers，已入 45 个 fixture 的判据）。

## 迁移清单

| 环节 | 定位方式 | 写入 |
|---|---|---|
| 相对路径改写 | `ImportDeclaration` / `ExportNamedDeclaration` / `ExportAllDeclaration` 的 `source` | span 替换 specifier |
| 类型 import 过滤 | `ImportDeclaration.specifiers`、`import_kind.is_type()` | 重建整条语句（含 `import type` 拆分） |
| dialog builder | `ObjectProperty`（键 `builder`、值调用已注册组件） | span 替换调用为 `new X(this, …)` |
| `$r` 内联 | `CallExpression`（callee `$r`、唯一实参字符串） | span 替换为对象字面量 |
| `declare enum` 折叠 | `StaticMemberExpression`，只取最外层 | span 替换为字面量 |
| 包源码路由 | 同「相对路径改写」的 AST 版 | span 替换 specifier |

依赖收集（遍历原始 `.ets`）仍走 token 扫描：`.ets` 里的 `struct` 不是 TypeScript，解析器会直接拒绝，而那里只需要 strings。

## 读数

```
fmt        ok
clippy     ok
tests      247 passed; 0 failed
oracle     45 passed, 0 failed
artifacts  45 same, 0 differing
```

顺带修了两处测试并发缺陷：`kit_dir_with_renamed_binding` 与 `fixture_kit_dir` 都写在固定临时目录，多个测试并发读写同一文件，实测 12 次里 1 次失败；改成 `pid + 序号` 后连跑 12 次 `0 failed`。

## 仍在的文本判定（有意保留）

- `declares_as_type`：判断目标模块是否把某名字声明为类型，读的是目标源码的 token。
- `appears_outside_imports`：判断名字是否在 import 之外被引用，读 token 而非遍历引用。
- `drop_folded_bindings`：折叠后按行删掉 system import 里被折叠的绑定。
- `apply_replacements`：按 span 拼接文本，所有替换的唯一出口。

这四处都不做"结构猜测"，只回答"这个名字出现过吗""这段文本是什么"，因此没有改成 AST 的收益。