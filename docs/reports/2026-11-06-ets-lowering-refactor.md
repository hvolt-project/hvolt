# 组件路径走 AST，两条路径共用一个 printer

日期 2026-11-06。判据来自 `scripts/verify.sh` 与 `scripts/oracle/`：AST 对账 43 个 fixture、产物对账 43 个 fixture。

## 目标

组件路径（`@Component` / `@ComponentV2`）与 plain 路径（无组件声明的文件）走同一条流水线：

    mask → 解析 → 在 AST 上变换 → codegen 输出

两条路径共用同一个 printer，不再各自排版；43 个 fixture 的 golden 对比面在新实现上重跑并全绿。

## 结构

`src/build` 按职责分组，不再平铺：

| 目录 | 职责 |
| --- | --- |
| `ets/` | ArkTS 语言层：`types` `preprocess` `mask` `children` `build_body` `builders` `sendable` `api_available` `comments` `context` |
| `lower/` | 源文件 → 可编译 TS：`ets_component` `ets_plain` `ets_output` `ets_transpile` `ui_syntax` |
| `compile/` | 编译器面产物：`abc_compile` `ets_resources` `ets_declared` `ets_source_map` `component_index` `conformance` `coverage_map` |
| `artifacts/` | 装配输入：`module_json` `pack_info` `pkg_context` `hap_pack` `assemble_hap` `insight_intent` `syscap` `res_compile` `native_compile` `export_check` |
| `deps/` | 依赖与 SDK 事实：`npm` `npm_cache` `sdk_facts` `sdk_types` `pipeline` |
| 顶层 | `lex`（词法）、`ets_assemble`（装配入口）、`ets_compile`（门面） |

## 决定

1. **一个 printer。** `lower::ets_output` 是唯一配置 oxc codegen 的地方（`print_program` / `print_statements` / `print_module`）。组件路径与 plain 路径都把自己的语句交给它；没有任何 lowering 代码负责缩进、引号或语句分隔。
2. **一次解析。** `lower::ets_component::read_component_file` 返回一次解析的三样东西：prepared 文本（节点 span 的坐标系）、import 节点、组件表。import 由节点 span 重写，不再逐行扫描，所以跨行的 import 连同其中的注释原样保留。
3. **AST 上取树。** `ets::build_body` 从 `build()` 的 AST 读组件调用、`if`、`ForEach`、`LazyForEach`、子组件与 `@BuilderParam`；`ets::children` 先把子块改写成尾随回调，使 AST 可读。
4. **上下文按线程存。** 一处文件的发射上下文（页面坐标、V1 backstore 名表、`@Reusable`）放在 `ets::context` 的 thread-local 里：它在渲染一条 `build()` 体期间被读，测试并行 lowering 时互不覆盖。
5. **比较面按「语义」判。** `compare.js` 跳过 `ParenthesizedExpression`：printer 按优先级打最少的括号，参考实现按输入原样打括号，两棵算子树相同即判相同；其余（语句顺序、标识符、字面量、注释文本）逐项比较。

## 读数

```
fmt        ok
clippy     ok   （--all-targets -D warnings）
tests      245 passed, 0 failed
oracle     43 passed, 0 failed     （AST 级）
artifacts  43 same, 0 differing    （modules.abc 反汇编级）
```

## 逐项修掉的形状（本轮）

| 形状 | 机制 |
| --- | --- |
| 无 `else` 的 `if` | 参考实现仍生成一个空分支，分支 id 递增 |
| `$field` 实参 | 构造参数读 backstore（`this.__x`），`paramsLambda` 读访问器（`this.x`） |
| V1 重发给子组件的字段 | 只有 `@Prop` 与 `@ObjectLink`，`@Link` 由子组件直接共享父 backstore |
| 跨文件 `@Builder` | 调用点是 `name.bind(this)(args)`，不开视图帧 |
| `Button('label')` | 构造方法是 `createWithLabel` |
| `$$expr`、builder 引用参数 | 展开成 `{ value, changeEvent }` 与 `{ builder: () => … }` |
| `ForEach` 键与标志 | 无键三参；无键但生成器读 index 时写 `undefined, true, false`；两标志分别表示生成器与键函数是否读 index |
| `LazyForEach` | 根节点只构造一次；无键函数时走四参 |
| `.animation()` / `.gesture()` | 动画上下文在创建回调内开合并闭合；手势链在 `globalThis.Gesture` 内展开 |
| 节点注释 | 出现在创建前、首个属性前、`pop` 前三处，与参考实现的位置一致 |
| `@Sendable` | 指令进入生成的 `constructor()`，函数则进函数体首行 |
| `@Reusable` 调用点 | `observeRecycleComponentCreation` + `createRecycle`；组件自身属性只写首渲染 |
| 顶层 `@Builder` 参数 | 编译产物里是 `__name__` |

## 遗留

- `src/build/ets_compile.rs` 现在是门面：51 处调用点仍读它的转发项，clippy 无未使用告警。清掉它需要先把这些调用点迁到 `ets::*`，属于整理项。
- `.scratch/backup-build-reorg/`、`.scratch/probe-project/`、`.scratch/probe-tmp/` 是本次重构产生的临时物，未删除。