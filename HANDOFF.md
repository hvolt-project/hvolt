# Handoff — hvolt 自建 ArkTS 编译链

工作区：`/Users/hongyuncong/workspace/better-harmony-app-build`

## 目标

用 hvolt 自己的 Rust 实现取代 HarmonyOS SDK 的 ArkTS 编译链（ets-loader → etsTransform → es2abc），使 HAP 构建不再依赖 hvigor / node / JS 插件。

## 已完成

### 1. 编译转换规则的完整逆向

证据来自 hvigor 在 Surface 上构建出的 `modules.abc` 反汇编（`ark_disasm.exe`），不是推测。

- 渲染入口：`initialRender()` 内每个 UI 节点生成一段 `this.observeComponentCreation2((elmtId, isInitialRender) => {...}, Component)`，子节点在其后依次展开，最后 `Component.pop()`。
- 状态字段：`@State message: string = 'Hello World'` 生成 `this._message = new ObservedPropertySimplePU('Hello World', this, 'message')` 加 getter/setter。
- 方法集（九个，全部已确认）：构造器、`initialRender`、`rerender`、`setInitiallyProvidedValue`、`updateStateVars`、`purgeVariableDependenciesOnElmtId`、`aboutToBeDeleted`、`__resetStateVarsOnReuse__Internal`、`getEntryName`。
- 页面注册：`registerNamedRoute(() => new Index(), '', { bundleName, moduleName, pagePath, pageFullPath, integratedHsp, moduleType })`。
- `util.isInitialRender()`：初始渲染时 `false`。

### 2. Rust 实现

`src/ets_compile.rs`（652 行 = 代码 573 + 测试 78）：

- `preprocess` — `struct` → `class`，等长替换以保住 span 偏移，跳过注释与字符串。
- `rewrite_builders` — `Foo(args) { children }` → `Foo(args, () => { children })`，括号栈 + 大括号匹配。
- `parse_components` — oxc 解析，提取组件名、装饰器、观测字段、`build` 方法体。
- `observed_fields` / `build_body` — 从 class body 提取的辅助函数。
- `parse_build` / `call_chain_to_node` — 把 UI 链式调用拆成 `UiNode` 树。
- `emit_ui` / `emit_component` — 按逆向出的协议生成 JS。

CLI 命令在 `src/cli.rs` 的 `CompileEts`，接线在 `src/main.rs`：

```
hvolt compile-ets <input.ets> [--out FILE] [--bundle B] [--module M] [--page P]
```

### 3. 端到端验证

```
输入   /tmp/Index.ets（@Entry @Component struct Index + @State + RelativeContainer + Text + onClick）
命令   hvolt compile-ets --bundle com.example.hello_world --module src --page pages/Index
输出   /tmp/IndexGen.js
后端   es2abc.exe IndexGen.js --output IndexGen.abc  →  3384 字节
反汇编 函数集与 hvigor 产物一致
```

### 4. 质量闸

```
cargo fmt --all --check                 空输出
cargo clippy --all-targets -- -D warnings   Finished
cargo test                              229 passed; 0 failed
```

## 未决

### 已完成（2026-11-03 第二轮）

- `emit_component` 改为返回 `Result<String, String>`，`parse_build` 失败不再静默产出空 `initialRender`，错误带组件名传到 CLI 退出码 1。
- 生成形态与 hvigor 产物逐项对齐：字段双下划线 `__x`、`if (p6 === undefined) p6 = -1`、`super(p3, p5, p6, p8)` 四参、`paramsGenerator_` 钩子、`setInitiallyProvidedValue(p2)`、`finalizeConstruction()`、`pageFullPath = '<module>/src/main/ets/<page>'`。
- 观测类按「装饰器 × 类型」二维选型，与 `ets-loader/lib/process_component_member.js` 的 `updateObservedPropertyPU` / `updateSynchedPropertyOneWayPU` 一致：
  - `@State` / `@Provide` / `@Consume`：简单类型 `ObservedPropertySimplePU`，对象类型 `ObservedPropertyObjectPU`
  - `@Prop`：`SynchedPropertySimpleOneWayPU` / `SynchedPropertyObjectOneWayPU`
  - `@Link`：`SynchedPropertySimpleTwoWayPU` / `SynchedPropertyObjectTwoWayPU`
  - `@ObjectLink`：`SynchedPropertyNesedObjectPU`
  - `simpleTypes` 判据抄自 loader：`string` / `number` / `boolean` / 枚举；字面量联合按简单处理
- `@ComponentV2` 显式报错退出，不再被当作 V1 组件生成 `ViewPU` 代码。

- `if / else if / else` 已支持：`UiItem::If` 按源码顺序编号分支，生成 `If.create()` + `this.ifElseBranchUpdateFunction(n, () => {...})` + `If.pop()`，外层套 `observeComponentCreation2(..., If)`。`If.create()` / `If.pop()` / `branchId` 的常量取自 `pre_define.js`，分支形态取自 `process_component_build.js` 的 `createIfBranchFunc`（PU 模式分支）。

### 功能缺口

- `@Provider` / `@Consumer`：属 V2 体系（`ViewV2` 协议），与 V1 的 `ViewPU` 成员集不同，需要单独一轮逆向，不在 V1 映射范围内。
- `ForEach` 已支持：`UiItem::ForEach` 在通用调用链之前特判，生成 `ForEach.create()` + `const forEachItemGenFunction = (item, index) => {...}` + `this.forEachUpdateFunction(elmtId, arr, gen, keyFn, true, false)` + `ForEach.pop()`。参数从箭头函数取名字而非源码切片，类型注解不进入产物（`es2abc` 不接受 TS 语法）。
- `import` 已保留：`parse_components` 收集 `Statement::ImportDeclaration` 的源码，`main.rs` 在产物最前输出，于是 `es2abc` 会编出 `MODULE_REQUEST_ARRAY`。**调用 es2abc 必须带 `--module --merge-abc`**（官方只加 `--merge-abc`，因为 ets-loader 的输出带 `export`；我们的输出没有，需要 `--module` 才会按 ESM 解析，否则报 `'import' and 'export' may appear only with 'sourceType: module'`）。
- `$r('app.media.x')` **未支持**。形态已取证：编译成 `createobjectwithbuffer { id: <number>, type: <20000=media>, params: [], bundleName, moduleName }`。
  id 不必自己算——`restool` 是原生 Mach-O 二进制（`toolchains/restool`），**不需要 JRE**，可以直接驱动：
  `restool -l <resConfig.json> -o <out> -r <out>/ResourceTable.txt`，`resConfig.json` 由 hvigor 生成在 `entry/build/default/intermediates/res/default/resConfig.json`。
  它会 print `restool resources compile success`，并把表写到 `resConfig.json` 的 `output` 字段所指目录（`-o` 不生效）。表形如 `media startIcon 0x01000009`，序号按名字的 ASCII 序分配（实测新增 `Mmm_mid`/`aaa_first`/`zzz_last` 得到 0x0c/0x0d/0x0e）。
  **已实现**：`src/ets_resources.rs`（`parse_table` + `rewrite`，5 测试），CLI 用 `--res-table <ResourceTable.txt>` 启用。
  `type` 码取自 `resource.d.ts`：color 10001、float 10002、string 10003、plural 10004、boolean 10005、intarray 10006、integer 10007、pattern 10008、strarray 10009、media 20000、rawfile 30000、symbol 40000。
  **顺序很关键**：`$r` 必须在 `rewrite_builders` **之后**替换，否则对象字面量的 `{ }` 会被当成 builder 子块；替换又是变长的，所以在 `parse_components_with` 里对 `prepared` 原地替换，再切片。
  实测：`$r('app.media.startIcon')` → `{ id: 16777225, type: 20000, ... }`，`$r('app.string.module_desc')` → `{ id: 16777222, type: 10003, ... }`，与官方 `modules.abc` 里的 `createobjectwithbuffer` 值一致。
- `LazyForEach` 已支持。形态取自官方产物：`LazyForEach.create("1", this, source, (item) => {...}, (item) => key)` + `LazyForEach.pop()`，其中 item 生成器的体是
  `const itemCreation2 = (e,i) => { <root>.create((e,i) => {<children>}, false); }` 加 `const observedDeepRender = () => { this.observeComponentCreation2(itemCreation2, <root>); <root>.pop(); }` 再 `observedDeepRender()`。
  归一化后与官方的差别只有命名：官方用具名函数 `__lazyForEachItemGenFunction` / `__lazyForEachItemIdFunc`，我们内联箭头，es2abc 因此编成 `@2*#^2` / `^3`；结构一一对应。
- 普通 `class` 会被原样保留（此前被整个丢弃），并在 `strip_ts_syntax` 里剥掉 `private`/`public`/`protected`/`readonly`、属性与参数的类型注解、返回类型、`implements` —— `es2abc` 把 `private` 当保留字直接报错。
- `@Builder` 已支持：`mask_builder_decorators` 用等长空格遮掉 `@Builder`（oxc 不接受函数声明上的装饰器），同时记下函数名；函数体按 UI 树生成成裸 `function name(params) { this.observeComponentCreation2(...) }`，调用处生成 `name.bind(this)(args)` 且不另开组件帧。
- `@Styles` / `@Extend` 已支持，规则由本机 hvigor 产物反推（见下节）。`@Extend(Text) f(i){ .fontSize(i) }` → 生成 `function __Text__f(i) { Text.fontSize(i); }`，调用处 `.f(20)` → `__Text__f(20)`；`@Styles function card(){ .width('100%') }` → 不生成函数，调用处直接内联成 `<调用点组件>.width('100%')`。两者函数体都是 `.attr(...)` 链，不是合法 JS，所以 mask 阶段把整个函数（含花括号）替换成等长空格，函数体只从 `parse_attr_chain` 重新生成。
- 函数名前缀差异：真实产物 `getEntryName` 为 `#~@0<#`，我们生成 `#~@0>#`；且真实产物多三个闭包函数 `#~@0>@1*#`。属 es2abc 修饰符与内联策略，需单独取证。

## 对账：与 hvigor 产物的逐函数对比

本机（macOS）有完整 DevEco Studio 26.0.0.461，可自己构建取证，不必再走 Surface。

构建命令（三处缓存必须重定向，否则沙箱 EPERM）：

```
export PATH="/Applications/DevEco-Studio.app/Contents/tools/node/bin:$PATH"
export DEVECO_SDK_HOME=/Applications/DevEco-Studio.app/Contents/sdk
export HVIGOR_USER_HOME=<工作区>/.scratch/hvigor-home
export npm_config_cache=<工作区>/.scratch/npm-cache
export JAVA_HOME=/Applications/DevEco-Studio.app/Contents/jbr/Contents/Home
/Applications/DevEco-Studio.app/Contents/tools/hvigor/bin/hvigorw assembleHap --no-daemon
```

产物：`entry/build/default/intermediates/loader_out/default/ets/modules.abc`
反汇编：`/Applications/DevEco-Studio.app/Contents/sdk/default/openharmony/toolchains/ark_disasm`
独立编译：`.../ets/build-tools/ets-loader/bin/ark/build-mac/bin/es2abc`

对比页覆盖 `@State` / `@Provide` / `@Extend` / `@Styles` / `@Builder` / `if-else` / `ForEach`。
函数名集合：官方 32 个，我们 31 个，**31 个完全同名**。唯一差异：

- 官方多一个空匿名函数 `#*#^1`（体内只有 `returnundefined`），是 `@Extend` 改写后的残留；我们直接生成 `#*#` 即 `() => new Index(...)`，语义相同。

已对齐的差异（本轮修掉）：

- `@Provide('theme')` 带参数时 `decorator_name` 返回 `None`，字段被静默丢弃 → 现在会解包 `CallExpression` 的 callee
- 方法名 `__resetStateVarsOnReuse__Internal` → `resetStateVarsOnReuse`（前者是 `@Reusable` 组件的名字）
- `getEntryName` 函数名前缀官方为 `#~@1<#`，我们为 `#~@1>#`；这是 es2abc 按访问修饰编码，不影响调用

## hvigor 直驱链进度（2026-11-03）

目标：hvolt.toml 直驱 → HAP 六文件（modules.abc / module.json / resources.index / resources/ / pack.info / pkgContextInfo.json），不复刻 hvigor 中间产物架构。

### module.json ✅ 已完成

`src/module_json.rs`（新模块）：`render(RenderInput)` 直接从 AppConfig + ModuleConfig 渲染最终形态 module.json，资源 ID 从 restool ResourceTable 注入。

- CLI：`hvolt render-module-json <project> --res-table <ResourceTable.txt> [--module M] [--build-mode debug|release] [--compile-sdk-version X] [--api-release-type Y]`
- 对账（asr-ondevice，对照 hvigor 的 package/default/module.json）：**1 处差异 = srcEntry**（我们 `./EntryAbility.ets` 扁平布局 vs hvigor `./ets/entryability/` 翻译路径）——设计差异，留到编译任务对齐 es2abc recordName 时验证。其余字段全等。
- 关键规则（从 hvigor 源码追出）：
  - API 编码：`merge-profile.js resolveApi` → HarmonyOS 用 `apiTransform`（每段补 2 位 + 主版本补 3 位，`"26.0.0"+26 → "26"+"00"+"00"+"026" → 260000026`）；OpenHarmony 用裸版本 26
  - `virtualMachine` 固定 `ark24.0.0.0`（probe 与 asr 两个工程实测一致，非 SDK 版本派生）
  - `buildVersion` 空时 = "1"（DevEco 默认）
  - skills 空时注入默认 home skill；action 按 runtime 区分：OpenHarmony `action.system.home` / HarmonyOS `ohos.want.action.home`
  - 模块 icon 空时回落 app.icon
  - `dependencies` 是空数组（不是空对象）
- 测试：3 个（api_version_encoding_follows_runtime / resource_references_become_ids / src_entry_is_flat）

### pack.info ✅ 已完成

`src/pack_info.rs`（新模块）：`render(PackInput)` 从同一份 manifest 派生 summary+packages 两段。复用 module_json 的渲染结果取 mainAbility/deviceTypes/abilities/distro。

- CLI：`hvolt render-module-json ... --pack-info` 追加输出
- 对账（asr-ondevice）：**与 hvigor 产物完全相等（0 差异）**，含 apiVersion 的 compatible/target 编码、packages name `src-default` 拼接
- 测试：2 个（the_summary_carries_the_installer_fields / the_package_name_joins_module_and_product）

### pkgContextInfo.json ✅ 已完成

`src/pkg_context.rs`（新模块）：`render(PkgContextInput)` 产出运行时依赖查找表。

- CLI：`hvolt render-module-json ... --pkg-context` 追加输出
- 字段规则（从 hvigor `generate-pkg-context-info.js` 逐条追出）：
  - entry 自身：packageName=模块名，entryPath=""（扁平布局——hvigor 标准布局是 src/main/，直驱差异）
  - native 依赖：`{name}.so` 条目，entryPath = types 去 `.d.ets`/`.d.ts` 后缀（"Index.d.ts"→"Index"），version 为空（hvigor 的 getVersion 只填 HAR）
  - on-disk .so：`libs/<abi>/*.so` 扫出，entryPath 空
  - dev 依赖（hypium/hamock）：不渲染——无 oh-package.json5，测试依赖不进正式包
- 对账（asr-ondevice）：`asrprobe.so` 与 `libasrprobe.so` **完全相等**；`src` 的 entryPath 是直驱差异
- 测试：3 个

### resources 编译 ✅ 已完成

`src/res_compile.rs`（新模块）：直驱 restool（SDK 原生编译器，无 JRE）。

- CLI：`hvolt res-compile <project> [--module M] [--out DIR]`（默认输出 `<project>/build/hvolt-out/res`，沙箱外工程可 --out /tmp/...）
- 流程：stage 资源树（project resources/ + 生成的 profile/main_pages.json）→ 自造 resConfig.json → restool -l 驱动 → resources.index / resources/ / ResourceTable.txt/.h/.ts
- 关键规则（踩坑实录）：
  - resConfig.json 的 `moduleResources` 必须指向 **stage 树**（含注入的 main_pages.json），不是项目源目录
  - resConfig.json 放 out_dir（放 stage 里会被 restool 当资源目录扫）
  - `applicationResource`（AppScope/resources）存在才写该字段；asr 直驱工程无 AppScope → 省略，ID 布局从模块资源 0x01000000 起（hvigor 因 AppScope 先声明 app_name 占 0x01000000）——语义自洽，直驱差异
  - `definedIds` 首次构建写 `{"record":[]}`（空对象报类型错、缺文件报打开错）
  - restool 路径：`sdk_root()/default/openharmony/toolchains/restool`（sdk_root 返回 sdk/ 根，要带 default/ 一级）
- 对账（asr-ondevice）：资源产物与 hvigor 同构（main_pages.json 逐字节一致、media 全齐、ResourceTable ID 全对上模块资源引用）
- 测试：1 个（main_pages_strips_ets_and_keeps_order）

### 装配 ✅ 已完成

`hvolt assemble-hap <project> [--module M] [--out DIR]`：一条命令出五件套 + rpcid.sc。

- `src/syscap.rs`（新模块）：device-define 交集 → rpcid.json → syscap_tool → rpcid.sc
  - 规则（从 `abstract-syscap-transform.js` 追出）：交集按**第一个 device 的 JSON 顺序**（Set 保序）；`rpcid.json` 键名是 `api_version` + `syscap`（不是 apiVersion/systemCapability——写错键 syscap_tool 报 get "syscap" object failed）
  - 对账（asr）：rpcid.sc **52742B 与 hvigor 逐字节相等**
- `pack-hap` 加 `--hvolt-out`：从 assemble 目录直接打包；rpcid.sc 存在才打进（hvigor 的 existsSync 门控）
- `hap_pack::inputs_from_hvolt_out`：模块根找 module.json/resources.index/resources/pack.info/pkgContextInfo/rpcid.sc
- 对账（asr，对照 hvigor 的 src-default-unsigned.hap）：**唯一缺口 = ets/modules.abc + libs/arm64-v8a/{libasrprobe,libc++_shared,libonnxruntime}.so**，正是 todo 6/7；无多余条目
- 测试：259 全绿（+1 syscap 排序辅助）

### es2abc 合并接入 ✅ 已完成（编译任务一段）

`src/abc_compile.rs`（新模块）：驱动 SDK es2abc 的 merge 阶段产出 modules.abc。

- CLI：`hvolt compile-abc <ts_root> <out.abc> [--file-threads N]`
- 流程：扫描展开后的 .ts/.ets/.js → filesInfo.txt（`<ts>;&<record>&;esm;<pkg>|<pkg>|1.0.0|<rel>;<pkg>;false;<ets|ts>`）→ npmEntries.txt（SDK 固定 7 行映射）→ `es2abc --merge-abc "@filesInfo" --npm-module-entry-list npmEntries --output modules.abc --file-threads N`
- 关键规则（实测与踩坑）：
  - **参数组合**：`--target-api-version 26 --enable-release-column --enable-callable-name` 与 hvigor 产物**语义零差异**（dump 4602 行 0 差异、大小 24268B 一致）；加 `--opt-try-catch-func=false` 反而破坏匹配
  - record 名：`<module>/<src/main/ets 相对路径>`（如 `src/src/main/ets/entryability/EntryAbility`）；ResourceTable 特化：`build/default/generated/r/default/` 两处 build-mode 段都去掉 → `src/build/generated/r/ResourceTable`
  - filesInfo 的 source 段去模块根级（`src/…` → `…`）；lang 标记 .ts→ets、.js→ts
  - `--target-api-version` 是**数字 26** 不是 "26.0.0"（写 "26.0.0" 报 out of range）
- 用户环境 SDK 在 `~/.hvolt/sdk/26.0.0/`（ts2abc.js 转发壳），es2abc 本体路径：`<sdk>/default/openharmony/ets/build-tools/ets-loader/bin/ark/build-mac/bin/es2abc`
- 测试：3 个

**尚未接入（编译任务二段）**：.ets → 展开 .ts 的转译（hvigor 用 rollup+arkts-compose）。验证用的展开产物来自 hvigor 缓存。现有 ets_compile 只处理 @Component 页面；EntryAbility 等普通类 + @kit.* → @ohos:* import 重写 + libasrprobe.so → @normalized:Y&&&libasrprobe.so& 都待接。这是剩下最大的独立块。

### native 编译 ✅ 已完成

`src/native_compile.rs`（新模块）：直驱 SDK cmake+ninja 编 native 模块。

- CLI：`hvolt build-native <project> [--out DIR]`（默认 `<project>/build/hvolt-out/libs`）
- 流程：`ohos.toolchain.cmake`（-DOHOS_ARCH=arm64-v8a -DOHOS_SDK_NATIVE）→ cmake -G Ninja → ninja → llvm-strip → 复制 `<out>/arm64-v8a/*.so`
- 工具路径：`<sdk>/default/openharmony/native/build-tools/cmake/bin/{cmake,ninja}` + `llvm/bin/llvm-strip`
- 对账（asr-ondevice）：**libasrprobe/libonnxruntime（.so.1 改名）/libc++_shared 三个产物与 hvigor stripped_native_libs 全部字节一致**
- 构建目录在 out 同级（.native-build-<name>），不进 libs/ 被 packer 扫到
- 测试：1 个

### 全链路验收 ✅（2026-11-03）

`assemble-hap + compile-abc + build-native + pack-hap --hvolt-out` 四步直驱 asr-ondevice：

- **HAP entry 与 hvigor 产物双向零差异**（无缺、无多）
- 大小：我们 121582007B vs hvigor 121581238B（差 769B = 直驱扁平 srcEntry + entry 顺序，可安装等价）
- modules.abc 语义零差异（dump 4602 行 0 差异）+ rpcid.sc 逐字节 + 三个 .so 逐字节 + restool 资源 ID 全对上

### 待办（todo list 驱动）

1. ✅ module.json
2. pack.info（summary+packages 两段，从同一份 module 信息派生）
3. pkgContextInfo.json（entry 自身 + 所有 path 依赖条目，含 native isSO=true）
4. main_pages.json 生成 + restool 产出 resources.index/resources/
5. 装配：产出到 build/hvolt-out/，pack-hap 改接该目录
6. 编译任务：es2abc 多文件合并接入（recordName OHM URL 格式：`@bundle:<bundle>/<module>/<path>`，需 --merge-abc + --npm-module-entry-list + --file-threads）
7. native：SDK cmake/ninja 编 .so，asr-ondevice 全链路

## 外部命令状态（2026-11-03）

已从 "调外部进程" 改为 Rust 内部实现：

| 命令 | 原调用点 | 现在的做法 |
|---|---|---|
| `hostname` | 设备协议模块的 `host_name` 兜底 | `platform::hostname()`：unix 走 `libc::gethostname`，Windows 读 `COMPUTERNAME` |
| `cp -a` | `tools.rs` 复制 ohpm | 复用已有的 `copy_tree_skipping()` |
| `curl` | 3 处（2 下载 + 1 POST） | `http::download_to_file()`（Range 续传 + 206 判定）+ `ureq` POST |
| `openssl` | `sign::check_cert_expired` | `x509-cert`（hapsigner 已带）+ `der`，读 `validity.not_after` |

计数（`grep -rho 'Command::new("<tool>")' src/*.rs | wc -l`）：hostname 0、cp 0、curl 0、openssl 0。

**仍是外部进程**：

| 命令 | 数量 | 原因 |
|---|---|---|
| `ssh` / `scp` | **4**（原本 17） | 已全部收敛到 `target.rs` 的四个函数：`ssh_run` / `ssh_spawn` / `scp_upload` / `scp_download`。换 `russh` 只改这四个 |
| 设备通道工具 | 8（`Command::new(&path)`） | `list targets` 与 `tconn` 属 client↔server 协议，设备协议模块只覆盖设备侧 |
| `hvigorw` / `ohpm` / `node` | 5 | SDK 自带工具，不属于待替换的自研命令 |

**设备通道替换进度**：连接类型的流已从 `TcpStream` 抽象成 `Box<dyn Transport>`；`pipeline.rs` 的装包与启动在 target 含 `:`（TCP）时走 设备协议模块的 `install` / `shell`，USB 序列号仍回落到外部工具。

**USB 传输层未动工**：连接类型只有 TCP；官方设备工具链的 USB 走 C++ FFI（`cffi/oh_usb.cpp`），没有纯 Rust 参考可抄。按裁决先拿 USB 侧拓包再写，没拓包不交付。

## Java 依赖状态

除打包顺序一处外，Java 已从构建链上摘除：

| 环节 | 工具 | 需要 JRE |
|---|---|---|
| ArkTS 编译 | `es2abc`（Mach-O） | 否 |
| 反汇编 | `ark_disasm`（Mach-O） | 否 |
| 资源编译 | `restool`（Mach-O），`-l resConfig.json` 驱动 | **否** |
| 签名 | `src/native_sign.rs`（275 行，7 测试） | **否** |
| HAP 打包 | `src/hap_pack.rs`（替代 `app_packing_tool.jar`） | 否 |

`hvigor` 那次 `Unable to locate a Java Runtime` 来自 `app_packing_tool.jar`；`hvolt pack-hap` 已不依赖它。

实测对账：`hvolt pack-hap <module_build> /tmp/ours.hap` 产出 **2698717 字节**，与 hvigor 的 `entry-default-unsigned.hap` **同样大小**，条目集合一致。哈希不同（`326dd369…` vs `a0de9e36…`）仅因条目顺序：`app_packing_tool` 把 `libs/<abi>/libentry.so` 排在 `libc++_shared.so` 之前，把 `resources/base/profile/` 排在 `resources/base/media/` 之前，都不是名字序。设备安装器读中央目录，两种顺序都能装。

## 环境

- Surface 主机：`xyzsu@100.64.0.2`
- SDK：`~/.hvolt/sdk/26.0.0/`
- `es2abc.exe`：`~/.hvolt/sdk/26.0.0/ets/build-tools/ets-loader/bin/ark/build-win/bin/es2abc.exe`
- `ark_disasm.exe`：`~/.hvolt/sdk/26.0.0/toolchains/ark_disasm.exe`
- 实验目录：`~/convtest/`（`Index.js`、`Index.abc`、`IndexGen.js`、`IndexGen.abc`、`real.pa`、`gen.pa`）

## 建议技能

- `harmony-build` — hvolt 构建、装包、抓日志的既有流程。
- `reverse-engineer-anything` — 继续从 `modules.abc` 逆 `@Prop` / `ForEach` / `@Builder` 的形态。
- `clean-code`（见 `docs/agents/clean-code.md`）— 三道闸与单函数长度上限。
- `arkts-pitfalls` — 写生成代码时避开 ArkTS 编译期限制。
