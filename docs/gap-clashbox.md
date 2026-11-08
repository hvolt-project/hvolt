# hvolt 编译 clashbox 的 gap 台账（2026-11-04）

目标：用 `hvolt build` 直接生成 clashbox 的 HAP，不使用 DevEco 工具链。
样本：`~/DshHome/clashbox-tailscale/clashbox`（`hvolt migrate` 从 DevEco 工程转出的工程），entry 模块 127 个 `.ets`。

## 判据

单文件转译扫描（`cargo test gap_scan -- --ignored`，`HVOLT_SCAN_ROOT` 指向 entry 目录）：

```
起始：SCAN total=127 ok=71  fail=56
阶段一：ok=108 fail=19
阶段二：ok=116 fail=11   ← 当前
```

扫描器：`src/build/ets_transpile.rs` 的 `gap_scan::scan_project`，对每个文件调用 `transpile`（组件走 `expand_component_file`，普通文件走 `transpile_plain`）。

## 已修（4 项，均有前后读数）

| # | 缺陷 | 判据 | 结果 |
|---|---|---|---|
| 1 | `@Builder export function X()` 未被识别为 builder 声明，装饰器残留在 `export` 前 | `mask_builder_decorators` 只接受 `@Builder` 后紧跟 `function` | ok 71→77 |
| 2 | `@Builder Name(...)` 方法声明被当作组件调用改写，参数表被换成 `, () => {` | `component_name_before` 只看首字母大写 | 并入 3 |
| 3 | `export struct` 在 oxc 里是 `ExportNamedDeclaration` 包裹的 class，组件收集只认裸 `ClassDeclaration` | 两个解析器（`ets_compile::parse_components_with`、`ets_component::parse_components`）都不认导出形态 | ok 77→96 |
| 4 | `Flex({…}) // 注释\n{ … }` 的子块未被识别，因为 `)` 与 `{` 之间有注释 | 只跳过空白，不跳注释 | ok 96→108 |
| 5 | 项目里的 `struct` 可以小写（`toggleRowList`），调用时被漏掉；组件名靠「首字母大写」判断不可靠 | 改为「SDK 内置组件名 + 项目 struct/builder 名」白名单（SDK 名单取自 `ets-loader/components/*.json`，141 个） | ok 108→116 |
| 6 | `$$this.x` 双向绑定语法、`AppStorage.get<boolean>(…)` 显式泛型参数不是合法 TS | 新增 `strip_two_way_binding` / `strip_generic_call_args`（跳过字符串与注释） | 见下 |

## 剩余 11 个失败文件（阶段二）

```
components/Configuration/ConfigAdd.ets      components/Settings/Appearance.ets
components/Configuration/ConfigInfo.ets     components/Settings/Backgrounder.ets
components/Home/Shortcuts.ets               components/Settings/Language.ets
components/More/AccessControl/AccessControl.ets  components/Settings/Notice.ets
components/More/Connect.ets                 pages/ConfigurationPage.ets
components/More/Diversion.ets               pages/HomePage.ets
components/More/Request.ets                 pages/Index.ets
components/More/Resources.ets               pages/PCIndex.ets
components/Proxy/ProxyArrangement.ets       pages/ProxyPage.ets
components/Proxy/ProxyGroupItem.ets
```

错误面：`Unexpected token` 8、`Expected a semicolon` 5、`build() 解析失败` 6。

已知实例的成因（EOX 偏移处的预处理产物）：

- `Diversion.ets`：`toggleRowList({…}) {…}` —— 跨文件 import 的 builder，名字小写且不在本文件 `CUSTOM_BUILDER_METHOD` 里，改写没覆盖。
- `ConfigAdd.ets`：`ForEach(...)` 与列表组合后的括号配对在文本改写后错位，报缺分号。
- `Connect.ets`：`aboutToDisappear` 前的 `}` 归属错乱。

## 造成该现象的原因

hvolt 现在是**文本改写**（`rewrite_builders` 做括号配对后插入 `, () => `），官方 ets-loader 是 **AST 变换**：

- 官方：`~/.hvolt/sdk/default/openharmony/ets/build-tools/ets-loader/lib/process_ui_syntax.js`
  - `processUISyntax()` 里对 `isStructDeclaration` → `processComponentClass` → `processComponentBlock(e.body, …)`
  - 顶层 `@Builder` 函数由 `COMPONENT_BUILDER_DECORATOR` 分支处理，builder 名登记进 `CUSTOM_BUILDER_METHOD`（`component_map.js`），改写按语法节点进行。
- 文本改写无法稳定区分「组件调用 + 子块」「builder 调用 + 子块」「普通调用」「跨文件 builder」，剩余 19 个失败都是这一类。

## 下一步的两种路线

A. 继续在文本改写上加规则（按当前速率，剩余 19 个文件逐个定位，但每修一类都可能引入新的误改，需回归）。
B. 把 builder/组件块改写改为基于 oxc AST 的结构变换，判据与官方 `processComponentBlock` 对齐（工作量集中在 `ets_component.rs` 的一个新 pass，之后文本层面的特例可以撤掉）。

两条路都要设备验证兜底：`clashbox` 目前 build 仍卡在 `ConfigAdd.ets`，尚未产出 HAP。
## 2026-11-04 夜间：入口 record 找不到 —— 变量排查矩阵

症状（恒定）：设备启动后 `Cannot find module '&entry/src/main/ets/entryability/EntryAbility&' , which is application Entry Point`。

| 内容 | 打包 | 签名 | 结果 |
|---|---|---|---|
| 官方 `outputs/entry-default-signed.hap` | 官方（原封不动） | 官方 | **能跑**（两次验证，pid 58321 / 10750） |
| 官方内容 | 官方 `app_packing_tool`（带 `--lib-path`，122 MB） | hap-sign-tool + debug 材料 | 崩 |
| 官方内容 | 我们 `hap_pack` | 同上 | 崩 |
| 我们的 abc | 官方 `app_packing_tool` | 同上 | 崩 |
| 我们的 abc | `hap_pack` | debug / AGC / 原始材料 / SHA384 | 崩 |

已逐项对齐但无效：

- record 名形式（点号/斜杠/版本后缀）：反汇编计数与官方完全一致（斜杠 2、点号 112）
- `srcEntry`、`module.json`（逐字段 diff）
- `pkgContextInfo.json`（换成官方那份）
- `api_transform` 编码（修正为 `60000023` / `50005017`）
- zip 条目 `flag_bits=2048`、4 字节与 4096 页对齐（abc 数据偏移对齐到 4096）
- 条目顺序（`ets/modules.abc` 均为第一）
- `versionCode` 变更（排除缓存）
- 卸载后干净安装
- 签名算法 SHA256（P-256）与 SHA384（P-384）
- 原生库缺失（补 `libs/*.so` 后仍崩）

同夜修掉的真实缺陷（与本问题独立，但都是 bug）：

- `collect_ets` 只收 `.ets`，漏掉全部 `.ts` 源（12 文件 / 25 record）
- 从未生成 `BuildProfile`，而 `EntryAbility` 第一条 import 就依赖它
- `@LocalBuilder` 方法被当作调用改写、`@Concurrent` 无法解析
- 装饰器体（`@Extend`）里注释中的 `}` 破坏块匹配
- `.d.ets` 未排除、根目录重复副本进 abc

未解：官方原包与任何重打包产物之间，仍有一处我无法复现的差异。

## 2026-11-05 自研链路：从「入口找不到」到「组件导出缺失」

每一条都由设备崩溃日志的**变化**确认（取 `jscrash-*.log` 的 `Error message`）。
工程检查项：`cargo fmt --check` 无差异，`cargo test` 185 项通过，`cargo clippy` 无 error。

| # | 缺陷 | 判据 |
|---|---|---|
| 1 | 入口模块 record 带版本号 | 基准 `&entry…EntryAbility&` 无版本；改后入口错误消失 |
| 2 | 生成的 `Index.ts` 未参与 import 改写 | abc 里 `module_request: ./X` 从 187 降到 0 |
| 3 | 多行 import / 多行 kit import 漏处理 | 扫描式改写 + 跨行拼接；`@kit` 残留 0 |
| 4 | 系统模块未写成归一化形式 | `@ohos.file.fs` → `@ohos:file.fs`；最小复现由崩转跑 |
| 5 | kit 配置只读一个 SDK 根 | `@kit.DeskTopExtensionKit` 仅在 hms；合并后解析为 `@hms:pcService.statusBarManager` |
| 6 | 第三方包 url 缺源根段、record 用点号 | `yaml/browser/dist/index` 与 record 对齐后命中 |
| 7 | 成员包入口路径 | 设备要 `&xb_components/src/main/ets/Index&1.0.0` |
| 8 | 纯类型导入/再导出被当值 | 按目标模块 `interface`/`type` 声明剔除（含 `export { } from`） |
| 9 | 一个文件多个 struct 只展开第一个 | 逐个展开；`Xb_Button.ts` 两个类都在 |
| 10 | 文件末尾 `export { A, B }` 丢失 | 解析结束后附给最后一个组件 |
| 11 | `export struct` 转 class 丢 `export` | `export class ButtonBuilder` |
| 12 | `@CustomDialog` struct 未被当组件 | `Xb_CustomDialog` 与 `IconLogo` 都生成 |
| 13 | byte-code HAR 无法处理 | `--enable-abc-input` 合并其 abc（10 个 record） |
| 14 | 工程自带的包索引被 `export *` 覆盖 | 模块有 `Index.ets` 时不再生成 barrel，别名导出得以保留 |
| 15 | `@arkts.*` 前缀未归一化 | 加入冒号化列表 |

**仍阻断启动**：`KeyCodeSelector` 由 `VirtualKeyboard` 未导出——与 #8/#14 同类（导出/链接不匹配），逐个暴露。

## 2026-11-05 续：导出与链接阶段的差异逐项减少

新增修复（每条都由设备崩溃信息的变化或静态读数确认）：

| # | 缺陷 | 判据 |
|---|---|---|
| 16 | 非组件的 `export class` 夹在 struct 之间被整段丢弃 | `KeyCodeSelector` 类重新出现 |
| 17 | `export { A, B }` 被当作 import 输出（关键字降级） | 内联路径与 `drop_one_import` 合一，保留 `export` |
| 18 | 包内同名入口（`index.ets`/`index.ts`/`index.js`）互相覆盖 | 清单声明的入口最后写入；hypium 的 `index.ts` 从 32 行桩变回 261 行实现 |
| 19 | 声明了但没被引用的包整包编入 abc | 只编被 import 的包；`@ohos/hypium` record 从 46 降到 0（基准也是 0） |
| 20 | 未使用的 import 不剔除 | 名称全文件未出现即删（再导出不适用） |
| 21 | 再导出里的类型名被当值 | 与 import 同一判据，`export { X as Y }` 保留 |
| 22 | 包上下文的 `entryPath` 用推导值而非清单声明 | 与基准逐字一致：`@ohos/hypium → index.js`、`yaml → browser/index.js`、`@linke/ui-color-picker → Index` |
| 23 | `@Builder` 方法原样输出（装饰器变成未定义的 `Builder` 调用） | 结构体级 `@Builder` 方法按 `build()` 同路径转换，签名取自 AST span；产物中 `@Builder` 残留 0 |

新增工具：`hvolt export-report <staged-tree>`，一次列出「导入了但目标没导出」的全部名字（当前 0 处）。

**当前阻断**：设备锁屏，`aa start` 被系统拒绝（`10106102`），无法继续用设备验证；静态侧已全绿。

## 2026-11-05 续二：设备启动链路的最后一层

又减少 6 项差异，每一项都以设备崩溃信息的变化作为判定依据，没有一项来自推测：

| 崩溃信息 | 缺陷 | 修法 |
|---|---|---|
| `State is not defined` | `@CustomDialog` 文件没进组件展开（`route::detect` 只认 `@Component`/`@ComponentV2`） | 加入 V1 路由；产物装配器残留从 6 降到 0 |
| `deviceInfo is not defined` | 属性读（`deviceInfo.sdkApiVersion`）不算值使用，被标成 `import type` | 值/类型判据改为「前一个非空字符是类型引导符吗」 |
| `LengthMetrics is not defined` | 静态调用 `Sym.member(` 在 `key:` 上下文被当成类型位置 | 静态调用一律判为值 |
| `KeyCode is not defined` | 枚举成员 `KeyCode.KEYCODE_A` 在对象字面量里形似类型位置 | 查 SDK 的 `declare enum`，枚举强制值导入（只对 enum 生效，避免与基准的 `import type window` 冲突） |
| `vKeyHeightSize is not defined` | 组件展开路径按行处理 import，**多行 import 整段丢失** | 补跨行拼接 |
| `LocalBuilder is not defined` | `@LocalBuilder` 方法原样输出 | 与 `@Builder` 同路径转换（调用点保持 `this.x()`） |

**当前阻断（设计未覆盖，需裁决）**：子组件实例化完全没有实现。

证据（同一文件、同一处）：

- 我们产出：`Xb_CircleButton.create({ ... })`（`Xb_TopTitleBar.ts:175`）
- 基准产物：`observeComponentCreation2` 的第二参数是 `{ 2 [ string:"name", string:"Xb_CircleButton", ]}`，即 `{ name: "Xb_CircleButton" }`

前者在运行时不成立（用户组件没有静态 `create`），后者是参考实现的子组件内联创建。这影响每一个嵌套组件调用点，是目前唯一挡住「界面能起来」的缺口。

## 2026-11-05 续三：子组件实例化（V1+V2）已完成

需求裁决：V1 与 V2 一起做。实现：

| 位置 | 内容 |
|---|---|
| `component_index.rs` | 编译前扫描全部源，得到「组件名 → 协议」，因为调用点无法从形状分辨用户组件与内建组件 |
| `ets_compile::emit_node` | 用户组件不再走 `Foo.create(...)`，改为 `observeComponentCreation2` + `new Foo(this, params, undefined, elmtId, () => {}, {})` + `ViewPU.create` / `ViewV2.create` + `paramsGenerator_` + `updateStateVarsOfChildByElmtId`，第二参数为 `{ name: "Foo" }`（与基准 disassemble 一致） |
| `ets_component` | 顶层 `@Builder function` 走展开流程（此前的实现把它落到 `top_level` 原样输出）；`export function` 保持 `export`；`export default X` / `export { }` 延后到类之后（否则 TDZ） |
| `npm::layout` | 有清单 `main` 的包以包根为源根：`@mcui/mccharts` 的 `index.ets` 在根部、组件在 `src/main/ets`，此前以 `src/main/ets` 为根会丢掉入口 |
| `rewrite_package_imports` | 包内系统模块同样归一化（`@ohos.display` → `@ohos:display`）；目录说明符补 `/index` |
| `drop_one_import` | `import Default, { A } from` 现在也参与类型剔除，并保留默认绑定 |

设备侧的验证信号：`.create(` 崩溃消失，`Resources is not initialized` 消失，模块加载推进到运行期。

**当前阻断（新子系统，需裁决）**：资源表与基准不一致。

| 读数 | 我们 | 基准 |
|---|---|---|
| `resources.index` 字节数 | 171 827 | 185 910 |
| 唯一字符串数 | 1 377 | 1 559 |
| 颜色值条目 | 35 | 48 |

`res_compile.rs` 里没有任何 `dark` 处理。基准缺的全是颜色字面量（`#0D474141` 一类），对应 `resources/dark/element/color.json` 这类**限定词目录**（dark / en_US / zh_HK / zh_TW）。表现是启动时 `getStringSync` 报 401「Invalid input parameter」——`$r(...)` 拿到的 id 在这张表里指不到字符串。

## 2026-11-05 续四：子组件的更新参数

`updateStateVarsOfChildByElmtId` 之前恒为空对象——父组件状态变化不会传到子组件。
现在按「子组件声明了该字段」∩「调用点确实传了该字段」生成映射：

```ts
this.updateStateVarsOfChildByElmtId(elmtId, { iconSize: this.iconSize, theBackgroundColor: this.theBackgroundColor, iconColor: this.iconColor });
```

`component_index` 同时记下每个组件声明的字段：V1 取 `@Prop`/`@Link`，V2 取 `@Param`；注释里的装饰器（`// @Param MenuList: …`）不算字段。
只发调用点绑定过的，避免把子组件的默认值覆盖成 `undefined`。

## 2026-11-05 续五：启动链路推到最后几层

设备侧现在能执行到真实的 `initialRender`（首个界面开始构建），此前所有失败都在模块加载期。逐条：

| 崩溃 | 缺陷 | 修法 |
|---|---|---|
| `getStringSync` 401 | 依赖模块的资源没进表（`dependencies: []`，`moduleNames` 只有 entry） | 按布局发现依赖资源目录并给绝对路径；`resources.index` 171 827 → 186 013 字节（基准 185 910） |
| `getStringSync` 401（同一处） | `$r('app.string.x')` 没在编译期解析成对象字面量 | 接入 `ets_resources::rewrite`，产出 `{ id, type, params, bundleName, moduleName }`；产物里 `app.string.add_config` 字符串消失 |
| `class constructor cannot called without 'new'` | `CustomDialogController({ builder: Foo({…}) })` 里的组件调用 | `builder:` 后的组件调用改成 `new Foo(this, {…}, undefined, -1, () => {}, {})` |
| `index is not defined` | 结构体级 `@Builder` 方法里的调用被 `bind_builder_calls` 提升到方法顶部，失去子块位置 | 只把独占一行的调用转成 builder 节点，参数里的调用保持在原位 |
| `CustomTab is not defined` | 结构体 builder 方法被当作顶层 `@Builder` 函数，调用点发成 `X.bind(this)()` | `Builder` 增加 `is_method`，方法类调用发成 `this.X(...)` |
| `onActionStart of undefined` | `.gesture(PanGesture({…}))` 没走工厂 | 按参考 disassembly 改成 `PanGesture.create({…})`（→ `tryldglobalbyname PanGesture; ldobjbyname create; callthis1`） |

**当前卡点**：`PanGesture.create({…})` 运行时返回 `undefined`，`.onActionStart` 报错。参考实现的调用形状与我们已经一致（全局取 `PanGesture` → 取 `create` → `callthis1`），所以差异不在调用形状，可能在参数的构造或该全局对象由谁提供。

资源索引剩余差 103 字节：基准多 `#FF4B6992`、`#FFAC49F5`、`agcit_ui_linke_uicolorpicker`（来自 `@linke/ui-color-picker` 这个**字节码 HAR**，它只带 `ResourceTable.txt`，没有 `resources/` 目录），我们多 `ClashTrayHolderAbility*`、`#1f1f1f`、`#FF202224`。

## 2026-11-05 续六：手势与首屏渲染

`hvigor` 的权威实现已在 `DevEco-Studio.app/Contents/tools/hvigor/hvigor-ohos-plugin/src/`：

- `builder/restool-file-config-builder.js`：`addModuleInputDir` → `moduleResources`，`addHarInputDir` → `dependencies`，`addAppInputDir` → `applicationResource`，`addModules` → `moduleNames`。我们现在的 resConfig 与这套字段一致。
- `lib/process_component_build.js` 的 `parseGesture` / `processGestureType`：

```js
// processGestureType
n.push(PanGesture.pop());
bindComponentAttr(...)                  // → PanGesture.onActionStart(cb)
n.push(PanGesture.create(源里的实参));   // 普通对象字面量
n.reverse();                            // create 在前
// parseGesture
Gesture.create(GesturePriority.Low) … 内层语句 … Gesture.pop()
// 且：不生成任何 .gesture(...) 属性调用
```

按此改正后，`gesture is not callable` 与 `onActionStart of undefined` 均消失。

**当前阻断**：`Cannot read property springMotion of undefined`。
`xb_components/utils/Animation.ts:15` 是 `import curves from "@ohos:curves"`，运行时设备报
`export objects of native so is undefined, so name is @ohos:curves`。
`@kit.ArkUI` 自身也是 `import curves from '@ohos.curves'`（默认导入），基准 abc 里 `@ohos.curves` 只有一条 `u8 @native.ohos.curves = 0x0` 的 native 记录。
`ls /system/lib64` 在设备上被拒（Permission denied），无法直接确认该系统模块是否可用。

## 2026-11-05 续七：应用起来了

设备侧读数：安装成功、`aa start` 后进程存活（pid 7922，持续 28 s 以上），`snapshot_display` 抓到界面（骨架屏 + 底部工具条），无新崩溃日志。构建链第一次产出可运行的应用。

本轮修的都是**用官方产物对照**定下来的（见下）：

| 崩溃或异常现象 | 造成该现象的原因 | 修改方式 |
|---|---|---|
| `springMotion of undefined` | 系统 native 模块前缀错误 | 按 loader `pre_define.js` 的 `NATIVE_MODULE` 集合，`ohos.curves`/`ohos.matrix4`/`ohos.app` 等走 `@native:`，其余走 `@ohos:` |
| `UploadSpeed.toString of undefined` | `@Consume` 字段没接线 | `this.__X = this.initializeConsume("KEY", "X")`，KEY 取 `@Consume('…')` 的实参 |
| `missing @Provide` | `@Provide('别名')` 未注册 | `this.addProvidedVar("别名", this.__X)`，别名与字段名不同时再补一条 `addProvidedVar("X", this.__X, true)` |
| `group_node is not defined` | 结构体 `@Builder` 调用被提出 ForEach 作用域 | 调用点就地发 `this.X.bind(this)(…)`；walker 新增 `UiItem::Raw`，未识别的语句原样保留 |
| `animation is undefined` | `.animation(…)` 不是组件静态方法 | 参考实现发 `globalThis.Context.animation(…)` |
| `create is undefined` | 跨文件的 `@Builder function` 被当成组件 | 全局登记所有 `@Builder function` 名，调用点发绑定调用 |
| `$configData is not defined` | `$x` 双向绑定的父侧写法 | 改成 `this.__x`（`$rawfile(…)`/`$r(…)` 是调用，不参与改写） |

**新增的能力：官方 hvigor 对照构建**
`/tmp/refstd-1793859661` 是以 `clashbox`（标准目录）为底、覆盖当前源码的副本；`hvigorw assembleHap` 在 `CompileArkTS` 之后才于 `PackageHap` 失败（本机无 Java），而 ets-loader 的产物已落在：

```
entry/build/default/cache/default/default@CompileArkTS/esmodule/debug/entry/src/main/ets/…
```

这是当前源码的权威 JS 输出，任何形状问题都可以直接 diff，不再靠 disassembly 反推。

### 验收方式修正：截图不能作判据，改用组件树

同一份运行（pid 7922）上，`snapshot_display` 的截图看起来像“骨架屏/模糊”，而 `uitest dumpLayout`（只读）给出的组件树是完整的：

```
Text 用户协议 [532,137][784,160]
Web resource:/RAWFILE/html/user_agreement.html [17,137][1299,2580]（含 heading/list/listItem）
Button 拒绝并退出 [81,2622][630,2762]
Button 同意并继续 [686,2622][1235,2762]
```

结论：应用正常停在用户协议页，WebView 文字在快照里被拍糊，不是渲染错误。
判据：界面状态一律以 `uitest dumpLayout` 的 `text`/`bounds` 为准，截图只作辅助。
