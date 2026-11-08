# proxy-node-item

真机工程里的一个 `@Reusable` 组件整体搬进来当 fixture。它第一次把「`@Reusable` + 属性写里读组件状态」
这一组合送进 oracle，量到的是 hvolt 与参考实现在 `if (isInitialRender)` 上的**拆分规则**不同。

## 来源

任务给的路径 `clashbox/xb_components/src/main/ets/components/Proxy/ProxyNodeItem.ets` 不存在；
本机有两份内容相同的副本（`diff -u` 输出 `IDENTICAL`，各 160 行）：

- `~/DshHome/clashbox-tailscale/clashbox/entry/components/Proxy/ProxyNodeItem.ets`
- `~/DshHome/clashbox-tailscale/clashbox/entry/src/main/ets/components/Proxy/ProxyNodeItem.ets`

`input.ets` 用前者原文（第 8 行注释起至文件末尾），只做了下面两处改动。

## 复刻

**1. 跨文件 import 换成 fixture 内的最小桩**（`@kit.*` 按约定保留）：

| 原文 import | 桩 |
| --- | --- |
| `EventHub` / `EventKey`（`common/EventHub`） | `class EventHub { static on(key, callback, once): void {} }` + `enum EventKey { TestDelay }` |
| `ProxyItem` / `ProxyData`（`common/datasources/ProxyData`） | `class ProxyItem { name; type; g }` + `class ProxyData { static getColorForLatency(delay): ResourceColor }` |
| `AppConfig` / `ProxyCardSize` / `UIConfig`（`entryability/AppState`） | `class AppConfig { proxyCardSize }` + `enum ProxyCardSize { Small, Large }` + `class UIConfig { stringDisplayIndex }` |
| `ClashViewModel` / `DelayInfo`（`entryability/ClashViewModel`） | `class ClashViewModel { static delayMap: Map<string, DelayInfo> }` + `class DelayInfo { name; delay }` |
| `customVibrator`（`common/utils/VibratorUtil`） | 原文未使用该符号，仍按桩留成 `const customVibrator: number = 0` |
| `hilog`（`@kit.PerformanceAnalysisKit`） | **保留原样** |

桩都放在组件之前、只留最少的成员，不参与任何断言：它们只是让文件能独立编译。

**2. 末尾补一个空 `@Entry` 组件。** `pages/Index.ets` 在 `main_pages.json` 里，hvigor 要求它
恰好带一个 `@Entry`，否则编译直接失败（删掉这一段的实测输出）：

```
1 ERROR: 10905402 ArkTS Compiler Error
Error Message: A page configured in 'main_pages.json or build-profile.json5' must have one and only one '@Entry' decorator. At File: .../ets/pages/Index.ets
```

这个没有方法体的 `ProxyNodeItemPage` 两侧都发射、内容一致，不影响下面任何一条结论。

## 量到的规则（golden）

同一份 golden 里，组件帧内共 43 条属性写（`@Extend` 函数体里的 8 条不在此列，两侧一致），
按「守不守 `if (isInitialRender)`」分成两组：

```ts
Text.create(delayText(this.delay));
Text.fontColor(ProxyData.getColorForLatency(this.delay));   // 读 this.delay → guard 外
if (isInitialRender) {
    Text.fontSize(12);                                      // 常量 → guard 内
    Text.maxLines(1);
    Text.textAlign(TextAlign.End);
}
```

* **guard 外 15 条**，全部在参数表达式里读 `this.X`：`fontColor(…this.delay)` ×3、`checked(this.item.name === this.selected)` ×3、
  `radioStyle({ checkedBackgroundColor: this.icon_emphasize })` ×3、`Row.width(this.uiConfig…)`、
  `fontSize(this.isPXSecScreen ? …)` ×2、`constraintSize({ maxWidth: this.uiConfig… })`、
  `textOverflow({ overflow: this.isPXSecScreen ? … })`、`textAlign(this.uiConfig…)`。
* **guard 内 28 条**，全部是常量表达式（数字字面量、`TextOverflow.Clip`、`'100%'`、`HorizontalAlign.Start` …）。
* 唯一一条「表达式里出现 `this.` 却仍在 guard 内」的是
  `Radio.onChange((isChecked) => { hilog.debug(… this.item.name …) })` —— `this.` 在**回调体**里，
  不在值表达式里。所以判据看的是属性值表达式本身，不递归进函数体。
* 分组会**重排**源码顺序：`Text`（else 分支）源码序是 `fontColor, textOverflow, layoutWeight, fontSize, maxLines, textAlign`，
  golden 发射成 `fontColor, textOverflow, fontSize, textAlign`（动态组，源码序）+ `guard { layoutWeight, maxLines }`（静态组，源码序）。

`@Extend` 调用（`__Text__ItemTextStyle()`）两侧都在 guard 之前，这条 hvolt 已经对了。

## hvolt 现状

同一次 `all.sh` 的输出：

```
FAIL fixtures/proxy-node-item
  AST differs
  comment text differs (20 vs 17)
```

* 43 条属性写**全部**落在 `if (isInitialRender)` 内（guard 外 0 条）。AST 逐节点比下来，两侧节点集合
  完全相同、只有顺序不同（把树按深度打印后排序比对，`diff` 为空），差异就是上面那 15 条的位置。
* 注释：`// group 不要写死…` 参考发射 4 份（`Radio.create` / `checked` / `radioStyle` / `hitTestBehavior`
  每条语句前一份），hvolt 只发射 3 份，且有一份落在 `}, Radio);` 之后、`Row.pop()` 之前；
  `//Item中type（节点类型）公用样式` 参考挂在 `__Text__ItemTypeStyle` 前，hvolt 挂到了 `delayText` 前。

## 复现

```sh
scripts/oracle/new-fixture.sh proxy-node-item
HOME=/tmp/hvigor-home scripts/oracle/run-hvigor.sh proxy-node-item
scripts/oracle/all.sh proxy-node-item          # FAIL，两条断言
diff -u fixtures/proxy-node-item/golden/Index.ts fixtures/proxy-node-item/actual.ts
```

计数用的三个脚本在 `.scratch/proxy-node-item/`（git 忽略）：`guards.py`（逐组件列出 guard 内外的属性写）、
`ast-dump.js`（按 `compare.js` 的归一化规则打印缩进树，用于定位结构差异）、`comments.js`（列出 `compare.js`
收集到的注释及其数量）。

## 覆盖边界

只比 `pages/Index.ts`：跨文件 import 已换桩，**跨文件**那一侧的形态（import 语句怎么被重写）不在本
fixture 内，由 `kit-*` / `import-*` 系列覆盖。