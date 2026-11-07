# comment-leading

The acceptance fixture for the comment-emission path: where the reference puts the comment a
source file writes at five positions that hvolt currently loses. Every marker is a unique token,
so the verdict is `grep -c` in `golden/Index.ts`, not a judgement call.

`golden/Index.ts` (232 lines) is the reference output for this input.

## Four forms in `build()`

| marker | source position | golden count | golden line | line above | line below |
|---|---|---|---|---|---|
| `// IF_START` | before `if (this.cond) { Text('a') }` | **1** | 149 | `If.create();` | `if (this.cond) {` |
| `// ELSE_FIRST` | first statement of the `else` block | **2** | 177 | `this.observeComponentCreation2((elmtId, isInitialRender) => {` | `Text.create('b');` |
| `// ELSE_FIRST` | (second copy) | | 180 | `}, Text);` | `Text.pop();` |
| `// FOREACH_START` | before `ForEach(this.items, …)` | **2** | 187 | `this.observeComponentCreation2((elmtId, isInitialRender) => {` | `ForEach.create();` |
| `// FOREACH_START` | (second copy) | | 198 | `}, ForEach);` | `ForEach.pop();` |
| `// AFTER_BLOCK` | after the `}` of a bare `if` | **3** | 210 | `}` | `else {` |
| `// AFTER_BLOCK` | (second copy) | | 218 | `this.observeComponentCreation2((elmtId, isInitialRender) => {` | `Text.create('d');` |
| `// AFTER_BLOCK` | (third copy) | | 221 | `}, Text);` | `Text.pop();` |

Three facts worth keeping in mind when judging a fix:

1. **`if` 前的注释落在 `If.create();` 与 `if (...)` 之间** — not before `If.create();`.
2. **无 `else` 的 `if` 也会有一个空 else 分支**：the reference emits
   `else { this.ifElseBranchUpdateFunction(1, () => { }); }` for a bare `if`, and `AFTER_BLOCK`
   is emitted immediately before that synthetic `else`. This is the source of the three copies;
   hvolt also emits the empty branch (the AST matches), it just does not emit the comment there.
3. **组件注释发两份**（`ELSE_FIRST` / half of `AFTER_BLOCK`）: once before `X.create(...)` inside
   `observeComponentCreation2`, once after the closing `}, X);` before `X.pop()`. `ForEach` gets
   the same pair around `ForEach.create();` / `ForEach.pop();`.

So **none of the four positions is "silent"** in the reference — every one of them fires.

## Member stacked comments

Source:

```
  /* 自定义弹框 */
  // controller 定义
  private dlg: CustomDialogController = new CustomDialogController({ … })
```

| marker | golden count | golden line | line above | line below |
|---|---|---|---|---|
| `/* 自定义弹框 */` | **2** | 71 | `this.__items = new ObservedPropertyObjectPU(['x']` | `// controller 定义` |
| `// controller 定义` | **2** | 72 | `/* 自定义弹框 */` | `, this, "items");` |
| `/* 自定义弹框 */` | | 140 | `}` | `// controller 定义` |
| `// controller 定义` | | 141 | `/* 自定义弹框 */` | `private dlg: CustomDialogController;` |

Both comments are emitted twice, and the first pair is **injected into the middle of the
neighbouring `@State`'s constructor argument list** (`new ObservedPropertyObjectPU(['x']` …
`, this, "items")`) — valid TypeScript, but not where the source wrote them.

**Why this is 2 copies and `configuration-page` measured 3 for its controller member**: that
member (`configCustomDialogController`) was *not* `private`, so it also appears in the synthesized
`_Params` interface and carries the comment there (see
`fixtures/configuration-page/golden/Index.ts:45-46`). A `private` member never reaches `_Params`,
so it gets two copies instead of three. The copy count is not a constant — it follows the
`_Params` membership.

## What hvolt emits today

`scripts/oracle/all.sh comment-leading` → `FAIL`, `comment text differs (29 vs 18)`, **AST blocks 0**.

| marker | golden | hvolt |
|---|---|---|
| `IF_START` | 1 | **0** |
| `ELSE_FIRST` | 2 | 2 (matches) |
| `FOREACH_START` | 2 | **0** |
| `AFTER_BLOCK` | 3 | 2 (missing the copy before the synthetic `else`) |
| `自定义弹框` | 2 | 1 (only the declaration, not the `ObservedPropertyObjectPU` injection) |
| `controller 定义` | 2 | **0** |

`ELSE_FIRST` matching is the one negative result that matters: a comment as the first statement of
an `else` block **does** survive today, so "first statement of a branch" is not by itself the rule.

## Two traps found while building this fixture

- **Zero-argument `@CustomDialog` breaks hvolt's output.** The first version wrote
  `builder: ProbeDialog()`; the reference emits
  `new ProbeDialog(this, {}, undefined, -1, …)` while hvolt emits
  `new ProbeDialog(this, , undefined, -1, …)` — the parameter object is missing, es2abc rejects the
  file (`SyntaxError: Primary expression expected`) and no `actual.ts` is produced. A plain
  component called with no arguments (`ConfigAdd()`) is fine on both sides, so the defect is
  specific to `@CustomDialog` (or to the `CustomDialogController` path). This fixture gives
  `ProbeDialog` a `@Prop` and passes `{ title: 'probe' }` to stay buildable; the zero-arg defect
  deserves its own fixture.
- `@CustomDialog` requires a `controller?: CustomDialogController` member or the reference refuses
  to compile the file at all.

## Using it as a gate

```sh
scripts/oracle/all.sh comment-leading     # must PASS once the comment path is fixed
grep -c IF_START        fixtures/comment-leading/golden/Index.ts   # 1
grep -c ELSE_FIRST      fixtures/comment-leading/golden/Index.ts   # 2
grep -c FOREACH_START   fixtures/comment-leading/golden/Index.ts   # 2
grep -c AFTER_BLOCK     fixtures/comment-leading/golden/Index.ts   # 3
grep -c '自定义弹框'      fixtures/comment-leading/golden/Index.ts   # 2
grep -c 'controller 定义' fixtures/comment-leading/golden/Index.ts   # 2
```

The golden line numbers above are for the current `golden/Index.ts`; regenerating it may shift
them, but the counts are the contract.
## 声明类型实参里的 carry：条件是「上一字段没有初值」

下一位成员的注释除了打在**构造函数语句**里（`this.__x = new X(…)` 的 `(` 之后，无条件），
在**声明类型实参**里还有第二份：

```ts
    private __proxyGroupItemSelected: SynchedPropertySimpleOneWayPU<ProxyCardSize
    //是否显示网络测速半模拟态判定
    >;
```

这一份的触发条件**(测出来的)不是装饰器种类、也不是泛型实参的类型**，而是**上一字段自身有没有初值**：

| 上一字段 | 有初值 | 声明里是否 carry |
| --- | --- | --- |
| `@Prop base1: number = 1` | 有 | 否 |
| `@Prop baseEnum: LocalKind = LocalKind.A` | 有（enum 类型也一样） | 否 |
| `@Prop baseNoInit: LocalKind`（父组件传参） | **无** | **是** |
| `@Consume('k') x: T`（无初值） | **无** | **是** |

旧反例也因此自洽：`@StorageProp('bottomRectHeight') bottomRectHeight: number = 0` 有初值，
所以它不 carry；三个 `@Consume` 成员都没有初值，所以都 carry。

**机制**：有初值时，类型的最后一个真实 token 是初值本身，参考的打印器把「声明文本段」记到
初值 token 的边界上，注释就落到别处；没有初值时类型是最后一段真实 token，而合成器在类型之后
追加 `>`，注释就落在 `>` 之前。

（探针标注：`NEXT_NOINIT` 复现了这条形态；`NEXT_STATE` / `NEXT_ENUM` / `MAT_*` 五组是反例对照。
落地点在 `process_component_member.rs` 的 `type_gap`。）
