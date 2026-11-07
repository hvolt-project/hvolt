# hvolt 对 OHOS SDK 的依赖面评估

采样时间 2026-11-06。对象：本仓 `26.0.0-8` 源码（`src/**`）。
实测样本：macOS 本机 SDK `v26.0.0.23`；Linux GPU 服务器 SDK `v26.0.0.38`（只读探测）。

## 结论

1. 依赖面很窄，但扎得很深：**6 个 SDK 内平台二进制 + 1 个 SDK 内 node 脚本 + 9 类数据文件 + 1 个远端 API**。
   编译器之外的 SDK 内容（hvigorw / ohpm / JDK / codelinter / 设备通道二进制）已经全部拔掉，
   与 `docs/reports/2026-11-04-purge-external-deps.md` 的裁定一致。
2. 耦合方式不是"调用 SDK 的 API"，而是**按路径就地取文件**：`sdk_root()/default/openharmony/...`
   这一条路径假设被 13 个硬失败点重复依赖，而 `doctor` 与 `sdk_facts` 各自维护了更宽容的形态探测——
   两套形态认知并不一致，这是依赖面里最大的结构风险。
3. 平台覆盖实际只验证了 macOS 与 Windows：`sdk_es2abc()` 只有 `build-win` / `build-mac` 两个分支，
   而 Linux SDK 的 `ark/` 下目录名是 `build`。当前链未在 Linux 上被验证过（服务器上那份 hvolt 是去外部依赖之前的旧构建）。

## 量化读数

| 指标 | 读数 | 取法 |
|---|---|---|
| 涉及 SDK 的源文件 | 30 个 | `grep -rl sdk --include=*.rs src \| wc -l` |
| SDK 引用行 | 462 行 | `grep -rn sdk --include=*.rs src \| wc -l` |
| 硬失败点（缺即 `Err`/退出） | 13 处 | 见下表 |
| SDK 内被执行的平台二进制 | 6 个 | es2abc / restool / syscap_tool / cmake / ninja / llvm-strip |
| SDK 内被 node 执行的脚本 | 1 个 | `ets-loader/bin/ark/ts2abc.js` |
| 被解析的 SDK 数据文件 | 9 类 | 见下表 |
| 远端接口 | 1 个 | `repo.harmonyos.com/sdkmanager/v5/ohos/getSdkList` |

## 契约清单

### A. 可执行体（缺即构建失败）

| SDK 内路径 | 消费者 | 用途 |
|---|---|---|
| `default/openharmony/ets/build-tools/ets-loader/bin/ark/{build-mac,build-win}/bin/es2abc` | [ets_assemble.rs:21-38](src/build/ets_assemble.rs#L21-L38)、[main.rs:324-335](src/main.rs#L324-L335) | ArkTS→`.abc` 合并，主链必经（[pipeline.rs:181](src/build/deps/pipeline.rs#L181)） |
| `default/openharmony/toolchains/restool` | [res_compile.rs:707-715](src/build/artifacts/res_compile.rs#L707-L715) | 资源编译，产出 `ResourceTable.txt` 与 id 注入版 `module.json` |
| `default/openharmony/toolchains/syscap_tool` | [syscap.rs:96-102](src/build/artifacts/syscap.rs#L96-L102) | 由 device-define 交集生成 `rpcid.sc` |
| `default/openharmony/native/build-tools/cmake/bin/{cmake,ninja}` | [native_compile.rs:30-42](src/build/artifacts/native_compile.rs#L30-L42) | 原生模块 configure/build |
| `default/openharmony/native/llvm/bin/llvm-strip` | [native_compile.rs:85-90](src/build/artifacts/native_compile.rs#L85-L90) | 原生 `.so` strip |
| `default/openharmony/ets/build-tools/ets-loader/bin/ark/ts2abc.js`（node 执行） | [sdk_facts.rs:64-80](src/build/deps/sdk_facts.rs#L64-L80) | 以 `--target-api-version` 问出字节码版本；**这是让 node 跑 SDK 的 JS 逻辑**，不是读数据 |

### B. 数据文件

| SDK 内路径 | 消费者 | 缺失时行为 | 绑定强度 |
|---|---|---|---|
| `ets/oh-uni-package.json`（`version`、`releaseType`） | [sdk_facts.rs:39-60](src/build/deps/sdk_facts.rs#L39-L60) | 读不到即 `Err`（`assemble_hap` 用 `?`） | 硬 |
| `sdk-pkg.json`（`data.version/apiVersion/releaseType/path`） | [manage.rs](src/sdk/manage.rs) 的 list/import/download | 管理命令报错 | 管理面 |
| `ets/build-tools/ets-loader/kit_configs/@kit.*.json`（`symbols.{source,bindings}`） | [ets_transpile.rs:87-130](src/build/lower/ets_transpile.rs#L87-L130) | `Err("kit_configs not found")` | 硬 |
| `ets/build-tools/ets-loader/sysResource.js`（就地文本解析） | [ets_resources.rs:56-108](src/build/compile/ets_resources.rs#L56-L108) | 空表，系统资源 id 解析失败 | 软 |
| `ets/api/@ohos.<module>.d.ts` | [sdk_types.rs:33-58](src/build/deps/sdk_types.rs#L33-L58) | `None`；且整模块 `dead_code` 休眠 | 软（当前无消费者） |
| `js/api/*.d.ts` | [ets_declared.rs:94-110](src/build/compile/ets_declared.rs#L94-L110) | `None`，少 enum 种类信息 | 软 |
| `ets/api/device-define/<device>.json`（`SysCaps`） | [syscap.rs:51-95](src/build/artifacts/syscap.rs#L51-L95) | 无该设备且无 `default.json` 即 `Err` | 硬（生成 rpcid.sc 时） |
| `toolchains/modulecheck/module.json`（deviceTypes 枚举） | [doctor.rs:552-566](src/project/doctor.rs#L552-L566)、[config.rs:695-712](src/support/config.rs#L695-L712) | doctor 降级为警告；常量兜底 | 软 |
| `hms/toolchains/id_defined.json` | [res_compile.rs:176-196](src/build/artifacts/res_compile.rs#L176-L196) | 写空记录占位 `{"record":[]}` | 软（有意设计） |

### C. 布局与网络

- **目录形态**：构建链只认 `sdk_root()/default/openharmony/{ets,js,native,previewer,toolchains}`。
  `sdk_root()`（[manage.rs:611-655](src/sdk/manage.rs#L611-L655)）返回"含版本目录的父目录"，各分支不统一取 parent。
- **版本命名**：安装目录 `v<version>`，`default` 指向当前版本；`sdk-ohos/<api>` 是 hvolt 自造的构建视图（[manage.rs:1106-1150](src/sdk/manage.rs#L1106-L1150)）。
- **远端 API**：`POST getSdkList`，body 为 `{osType, osArch, supportVersion}`，`supportVersion` 默认常量 `26.0-ohos-single-1`（[manage.rs:7](src/sdk/manage.rs#L7)）；平台映射只覆盖 darwin/arm64、linux/x64、linux/arm64、windows/x64（[manage.rs:762-772](src/sdk/manage.rs#L762-L772)）。
- **产物耦合**：`compileSdkVersion` 直接取 SDK 的四段版本，`virtual_machine` 取 `ark<字节码版本>`（[module_json.rs:62](src/build/artifacts/module_json.rs#L62)）。

## 风险与开放项

### R1（实测，未闭环）平台分支不覆盖 Linux

代码两处写死 `if cfg!(windows) { "build-win" } else { "build-mac" }`
（[ets_assemble.rs:26-33](src/build/ets_assemble.rs#L26-L33)、[main.rs:324-330](src/main.rs#L324-L330)）。

实测两边形态：

| 机器 | SDK | `ets-loader/bin/ark/` 下的子目录 |
|---|---|---|
| macOS（本机） | `v26.0.0.23` | `build-mac`、`ts2abc.js` |
| Linux（GPU 服务器） | `v26.0.0.38` | `build`、`ts2abc.js` |

推论（代码可机械判定）：非 Windows 平台若 `ark/` 下没有 `build-mac`，`sdk_es2abc()` 的 `find(|p| p.exists())` 必空，
返回 `Err("es2abc not found in SDK ets-loader")`，`hvolt build` 在主链第 181 行即中断。

未闭环的原因：服务器上的 `~/.local/bin/hvolt` 是**去外部依赖之前的旧构建**
（`--help` 仍有 `assemble`/`lint`/`tools`，`build` 自述 "invoke hvigorw"），不能用来证实或证伪新链。
因此这是"代码事实成立、运行时未验证"的开放项。

验证方式（只读，不写工程）：在 Linux 上对当前仓库构建出的二进制跑 `hvolt build`，
或临时把 `else` 分支改为按 `read_dir(ark)` 探测实际子目录。

### R2（结构）形态归一不一致

构建链 13 个硬失败点共享同一条路径假设；`doctor` 与 `sdk_facts` 却各自接受三种形态
（`default/openharmony`、`ets` 直挂、`openharmony` 直挂）。当 `sdk_path` 指向非 `default/` 布局时，
`doctor` 可能报健康而构建失败。这一不一致没有任何测试覆盖。

### R3（格式）对 SDK 内部 JS 的数据耦合

`sysResource.js` 被 hvolt 用逐行文本扫描解析（`module.exports.sys = {kind:{name:id}}`），
`ts2abc.js` 被 node 执行后读 stdout。这两者都是 ets-loader 的实现细节，不是对外接口；
DevEco 升级不保证其形状。`kit_configs` 的 `symbols.{source,bindings}` 同理。

### R4（版本）单版本假设

- `SDK_SUPPORT_VERSION = "26.0-ohos-single-1"` 是硬编码，新 SDK 代际需改代码或改配置覆盖。
- `compatible_api()` 用"括号内数字"或"首段数字"两种规则猜 API level（[sdk_facts.rs:88-105](src/build/deps/sdk_facts.rs#L88-L105)）。
- OpenHarmony 与 HarmonyOS 两套 deviceTypes 枚举以常量钉住（[config.rs:695-712](src/support/config.rs#L695-L712)），
  注释要求换 SDK 后重测。

## 可替换性判断

| 依赖项 | 能否自持 | 代价 |
|---|---|---|
| es2abc | 否（本仓无 ArkTS→abc 编译器） | 需接 oxc 之外的完整 Ark 编译器，工程量等同重做 es2abc |
| restool | 部分 | 资源编译规则与 `resources.index` 二进制格式可复刻，已有 `ets_resources.rs` 解析一侧 |
| syscap_tool | 是 | `rpcid.sc` 是 `rpcid.json` 的编码，格式已知（文件头注释记录过字节等价的实测） |
| cmake / ninja / llvm-strip | 是 | 换成系统工具链即可，但需保证 `ohos.toolchain.cmake` 的语义一致 |
| oh-uni-package.json / sdk-pkg.json | 是 | 纯数据，可内置 |
| kit_configs / sysResource.js / device-define | 是 | 需要在仓库内维护快照，代价是随 SDK 版本漂移 |
| ts2abc.js 的问询 | 是 | API level→字节码版本是一张可固化的表，但要按 SDK 版本分支 |

结论：真正不可替代的只有 **es2abc**；其余都是"数据 + 格式"，可以抽成 hvolt 自带的 SDK 快照，
把依赖面从"目录布局 + 六个二进制"收敛到"一个编译器 + 一份数据快照"。