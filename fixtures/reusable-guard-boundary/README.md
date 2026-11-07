# reusable-guard-boundary

`@Reusable` 组件的 `if (isInitialRender)` 到底包哪些属性写。`proxy-node-item` 只钉住了一条
（读 `@State/@Prop/@Consume/@StorageLink` 的属性写必须在 guard 外），本 fixture 把判据的
边界一类一类量出来。结论反直觉：判据不是「读不读 `this`」，不是「是不是常量表达式」，
也不是「类型是不是字面量」，而是**表达式的语法形状 + 符号的声明种类**。

## 输入结构

| 组件 | 测什么 |
| --- | --- |
| `PlainMemberProbe` | `this.count`（`private count = 3`，无装饰器）/ 模块 `const GAP` |
| `CallAndTernaryProbe` | `getColor(this.delay)` / `getColor(3)`（对照）/ `this.big ? 20 : 12` / `12 + this.delay` |
| `StyleProbe` | `@Styles` 常量体（3 条、2 条）与读状态体 |
| `BuilderProbe` | `@BuilderParam footer: () => void` |
| `ModuleVarProbe` | 模块 `let runtimeWidth` |
| `FormProbe` | 类静态成员 / 对象字面量三种值 / 一元 / 常量二元 / 模板串 / `this.method()` |
| `EnumProbe` | SDK 枚举 `TextAlign.End` / 本地枚举 `LocalKind.A` / 本地类静态 `Shadow.GAP` / 三者混排一帧 |
| `EnumMechanismProbe` | `const enum` / `as` 断言 / namespace 导出 / 枚举变量 / 字面量类型常量 / 类静态 readonly 与可变 |
| `MixOrderProbe` | `@Styles` 与普通属性写交替、两个 `@Styles` 连用 |
| `StylesScopeProbe` | `@Styles` 名字作用域（与 `StyleProbe.cardStyle` 重名、体不同） |

## 量到的归属（`golden/Index.ts` 行号）

| 值表达式 | 归属 | 行 |
| --- | --- | --- |
| `Text.margin(GAP)`（模块 const） | **guard 外** | L92 |
| `Text.fontSize(this.count)`（普通成员） | **guard 内** | L94 |
| `Text.fontColor(getColor(this.delay))` | guard 外 | L159 |
| `Text.fontColor(getColor(3))`（对照） | guard 外 | L164 |
| `Text.fontSize(this.big ? 20 : 12)` | guard 外（整条） | L169 |
| `Text.fontSize(12 + this.delay)` | guard 外 | L174 |
| `Text.width(this.span)`（`@Styles` 读状态体） | guard 外 | L224 |
| `@Styles` 常量体 → `padding(5); height(20); width(40)` | guard 内 | L226-228 |
| `Text.fontSize(Shadow.GAP)`（类静态成员） | **guard 外** | L374 |
| `Text.margin({ top: 2 })` | guard 内 | L380 |
| `Text.margin({ top: GAP })` | **guard 外** | L386 |
| `Text.margin({ top: this.plain })` | guard 内 | L392 |
| `Text.layoutWeight(-1)` | guard 外 | L398 |
| `Text.fontSize(2 * 3)` | **guard 外** | L403 |
| `Text.width(\`${this.plain}%\`)` | guard 外 | L408 |
| `Text.width(this.widthOf())` | guard 外 | L413 |
| `Text.textAlign(TextAlign.End)`（SDK 枚举） | guard 内 | L450 |
| `Text.fontSize(LocalKind.A)`（本地枚举） | guard 内 | L457 |
| `Text.fontSize(Shadow.GAP)`（类静态） | guard 外 | L463 |
| `enum-mix`：`width(Shadow.GAP)` 外 / `fontSize(LocalKind.A)`、`textAlign(TextAlign.End)` 内 | 同帧逐条判 | L468-471 |

`@BuilderParam` 不参与这个判定：`this.footer()` 发射成帧级的 `this.footer.bind(this)();`（L292），
无论 `@Reusable` 与否都在 guard 之外。

**guard 只在至少有一条静态属性时发射**：全动态的帧（`call` / `control` / `ternary` / `binary` /
`module-var` / `static-member` / `object-identifier` / `unary` / `binary-const` / `template` /
`call-this` / `as-cast` / `namespace` / `enum-var` / `const-auto` / `static-readonly` / `static-mutable`）
根本没有 `if (isInitialRender)`。

## A. 枚举成员 vs 类静态成员：判据是符号的声明种类

| 源码写法 | golden 发射成 | 归属 | 行 |
| --- | --- | --- | --- |
| `const enum ConstKind { A = 1 }` → `ConstKind.A` | **`1`**（被内联成字面量） | guard 内 | L524 |
| `(LocalKind.A as number)` | `(LocalKind.A as number)`（断言保留） | guard 外 | L530 |
| `namespace NsHost { export const X = 1 }` → `NsHost.X` | `NsHost.X` | guard 外 | L535 |
| `const kindVar: LocalKind = LocalKind.A` → `kindVar` | `kindVar` | guard 外 | L540 |
| `const AUTO = 1`（无注解，推断为字面量类型 `1`）→ `AUTO` | `AUTO` | guard 外 | L545 |
| `class StaticFields { static readonly READONLY_GAP = 3 }` → `StaticFields.READONLY_GAP` | 原样 | guard 外 | L550 |
| `class StaticFields { static mut = 4 }` → `StaticFields.mut` | 原样 | guard 外 | L555 |
| 对照：`LocalKind.A` / `TextAlign.End` | 原样 | guard 内 | L457 / L450 |

三条结论：

1. **`const enum` 根本没进判定**：TS 在发射前把 `ConstKind.A` 换成 `1`，落到判据面前时已经是
   数字字面量，所以「静态」是内联的结果，不是枚举成员被识别。声明本身仍原样保留（L481-484）。
2. **`as` 断言整条算动态**：`(LocalKind.A as number)` 里的操作数虽然是枚举成员，整条仍在 guard 外。
3. **namespace 导出、类静态字段（无论 `readonly`）、模块 `const` 一律动态**；`const AUTO = 1`
   的字面量类型也不救它。所以判据不是「类型是不是字面量/枚举字面量」，而是**符号是不是 enum member**。

## B. `@Styles` 与普通属性混排的顺序

参考实现的静态组不是源码序：**`@Styles` 展开的每条属性都排在静态组最前面，后调用的 `@Styles`
块更靠前，每个块内部再反序；普通属性写按源码序排在它们之后**。等价于：`@Styles` 展开项
逐条 `unshift`，普通属性 `push`。

`mix1`（源码 `.width(40).fontSize(this.dim).pairMixStyle().height(20).margin(GAP).padding(5)`，
`pairMixStyle` = `.width(70).height(80)`）：

```ts
Text.create('mix1');
Text.fontSize(this.dim);      // 动态组，源码序，guard 外
Text.margin(GAP);             // 动态组，源码序，guard 外
if (isInitialRender) {
    Text.height(80);          // pairMixStyle 反序，且整体提到普通属性之前
    Text.width(70);
    Text.width(40);           // 普通属性，源码序
    Text.height(20);
    Text.padding(5);
}
```

`mix2`（`.pairMixStyle().tripleMixStyle().width(90)`，`tripleMixStyle` = `.padding(1).margin(2).backgroundColor(Color.Blue)`）：

```ts
if (isInitialRender) {
    Text.backgroundColor(Color.Blue);   // 后调用的 triple 块在前，块内反序
    Text.margin(2);
    Text.padding(1);
    Text.height(80);                    // 先调用的 pair 块在后，块内反序
    Text.width(70);
    Text.width(90);                     // 普通属性最后
}
```

`mix3`（`.fontColor(getColor(this.dim)).pairMixStyle().fontColor(getColor(this.span))`）：

```ts
Text.create('mix3');
Text.fontColor(getColor(this.dim));   // 两个动态项按源码序
Text.fontColor(getColor(this.span));
if (isInitialRender) {
    Text.height(80);
    Text.width(70);
}
```

两条 `@Styles` 连用时每个块仍各自反序，块与块之间也反序。

## `@Styles` 的名字作用域（附加发现）

`StylesScopeProbe` 定义了自己的 `cardStyle()`（`.width(11).height(12)`），与 `StyleProbe.cardStyle()`
（`.width(40).height(20).padding(5)`）同名。参考按**自身所属组件**解析：

```ts
Text.create('scope');
if (isInitialRender) {
    Text.height(12);   // StylesScopeProbe 自己的体，反序
    Text.width(11);
}
```

hvolt 发射成 `width(40); height(20); padding(5)`（`StyleProbe` 的体）——它按名字查 `@Styles`，
同名跨组件串了。这与 guard 无关，是独立的缺口。

## 局部变量写不出来

`build()` 内声明局部变量直接被编译器拒绝（把 `LocalVarProbe` 放进去后的实测）：

```
1 ERROR: 10905210 ArkTS Compiler Error
Error Message: In an '@Entry' decorated component, the 'build' method can have only one root node,
which must be a container component. At File: .../pages/Index.ets:87:12
```

所以「属性值是一个局部变量」在 ArkTS 1.1 里不可构造；`ModuleVarProbe` 用模块级 `let` 做最近的替身。

## 复现

```sh
scripts/oracle/new-fixture.sh reusable-guard-boundary
HOME=/tmp/hvigor-home scripts/oracle/run-hvigor.sh reusable-guard-boundary
scripts/oracle/all.sh reusable-guard-boundary     # 现状：FAIL / AST differs
```

逐帧列 guard 内外归属用 `.scratch/proxy-node-item/guards.py`（git 忽略），它按
`observeComponentCreation2` 分帧、按 `if (isInitialRender)` 分母打印每条属性写的位置。

## 实现规格

判据与发射顺序给实现者用的规格在 [SPEC.md](SPEC.md)：判定函数签名、逐条规则 + golden 证据行号、
外部符号表依赖、发射算法与验收口径都在那里；本 README 只保留「量到了什么」。

## `@Styles` 的作用域（2026-11-07 实测，已修）

`@Styles` 按名字查找之前只在全文件范围里找第一条命中，于是同名声明会跨组件串用。golden 的
展开结果：

| 场景 | 声明 | golden 展开 |
| --- | --- | --- |
| 两个 struct 同名成员 | `StyleProbe.cardStyle` 40/20/5、`StylesScopeProbe.cardStyle` 11/12 | 各自展开自己的（`Text.height(12); Text.width(11);`） |
| 成员与顶层同名 | `globalStyle`（顶层 99/98）与 `GlobalStyleShadowProbe.globalStyle`（21/22） | **成员赢**：`Text.height(22); Text.width(21);` |
| 只有顶层 | `GlobalOnlyProbe` 调 `.globalStyle()` | 展开顶层：`Text.height(98); Text.width(99);` |
| 同 struct 两个不同名 | `StyleProbe.pairStyle` / `dynStyle`、`MixOrderProbe.pairMixStyle` / `tripleMixStyle` | 各按自己的体展开，互不串 |

实现：`StyleFn` 带 `comp`（成员是所属 struct 名，顶层 `@Styles function` 是空串），
`ets_component.rs` 按 `comp` 过滤后再把成员的排在顶层的前面，于是 `find` 优先命中成员、
没有成员时回落到顶层。`@Styles` 不跨 struct 继承，所以没有「子组件继承父组件样式」这种场景可测。

## 与 proxy-node-item 的关系

`proxy-node-item` 是真实文件的定案证据，不要改它。本 fixture 是纯探针：只放要测的形态，
不追求是真代码。两者一起才能说明「hvolt 现在把所有属性写塞进 guard」这一条缺口的边界。