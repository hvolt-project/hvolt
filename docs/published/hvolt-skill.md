# hvolt 技能 — HarmonyOS Community Build Tool

## 项目概述

hvolt（HarmonyOS Volt）是一个社区构建工具，将扁平化项目结构转换成 HarmonyOS 标准工程布局。

语言：Rust（edition 2021）
二进制：`hvolt`
仓库：`hvolt-project/hvolt`（公开社区项目）

### 扁平项目布局（hvolt 自己的工程格式）

hvolt 不只是构建工具，它**定义**工程格式：一棵扁平树，`assemble` 翻成标准 HarmonyOS 工程。
权威示例：`examples/hello-world`（最小）与 `examples/hello-native`（含 C++/NAPI 原生模块）。

```
<app>/
├── hvolt.toml              # 应用级：[app] bundle/name/version/vendor/icon/sdk_version/members、[app.signing]、[dependencies]
├── src/                    # 一个模块（目录名即模块名）
│   ├── hvolt.toml          # 模块级：[module] name/routes/entry/device_types/obfuscation
│   ├── EntryAbility.ets
│   └── pages/Index.ets
├── resources/base/         # element/{string,color,float}.json + media/
└── mylib/                  # 原生模块（可选）
    ├── hvolt.toml          # 顶层字段 name / type="native" / types / version / description
    ├── CMakeLists.txt      # add_library(<name> SHARED napi_init.cpp)
    ├── napi_init.cpp       # NAPI + __attribute__((constructor)) 注册
    └── Index.d.ts          # 给 ArkTS 的类型声明
```

**原生模块三条约定**（实测，违反则构建失败或运行期找不到库）：

1. 原生模块**只声明在 `[dependencies]`**，不写进 `app.members`（`is_native_module` 按 `type="native"` 判定）。
2. `mylib/hvolt.toml` 的 `name` 决定产物 `lib<name>.so`；CMake target、`nm_modname`、ArkTS `import` 必须同一致
   （`name="hello"` → target `hello`、`nm_modname="libhello"`、`import { add } from 'libhello.so'`）。
3. `types` 指向类型声明；`assemble` 生成 `src/main/cpp/types/<依赖名>/`（`oh-package.json5` + `Index.d.ts`），
   并在模块 `oh-package.json5` 里加 `"<依赖名>.so": "file:./src/main/cpp/types/<依赖名>"`，
   同时写 `buildOption.externalNativeOptions.path = ./src/main/cpp/CMakeLists.txt`、`abiFilters = ["arm64-v8a"]`。

## 架构

### 核心模块

| 模块 | 职责 | 关键函数 |
|---|---|---|
| `src/main.rs` | 入口，命令分发 | `main()` |
| `src/cli.rs` | CLI 定义（clap derive） | `Args`, `Command` enum |
| `src/assemble.rs` | 翻译用户结构→标准布局 | `assemble()` |
| `src/sdk.rs` | SDK 发现/管理/下载 | `detect()`, `list_remote()`, `download_sdk()`, `import_sdk()` |
| `src/pipeline.rs` | 构建流程编排 | `build_and_sign()`, `run()` |
| `src/build_hvigor.rs` | hvigorw/ohpm 调用 | `run_ohpm_install()`, `run_assemble()` |
| `src/sign.rs` | 远程签名（AGC API） | `sign()`, `sign_remote()` |
| `src/config.rs` | `hvolt.toml` 解析 | `AppConfig`, `ModuleConfig`, `NativeModuleConfig` |
| `src/tools.rs` | 工具发现（hvigorw, ohpm, node） | `find_hvigorw()`, `find_ohpm()` |
| `src/translator/` | 项目翻译策略 | `V26Strategy`, `translate()` |
| `src/translator_resources.rs` | 资源/ETS 文件复制 | `copy_resources()`, `copy_ets_files()` |
| `src/hdc.rs` | hdc 工具发现 | `find_hdc()` |
| `src/scaffold.rs` | 项目脚手架生成 | `scaffold()` |
| `src/lint.rs` | 代码扫描（官方 codelinter 驱动 + SDK 形态桥接） | `find_codelinter()`, `build_args()`, `ensure_open_sdk_overlay()` |
| `src/check.rs` | 包侧质量闸（官方 app_check_tool） | `check_package()` |
| `src/hap.rs` | HAP 内省（bundle/版本/权限） | `hap_info()` |
| `src/doctor.rs` | 环境/SDK/工具/设备/凭据一致性预检 | `run()` |
| `src/device.rs` | 设备管理（hdc 包装） | `list()`, `connect()`, `set_remote()` |
| `src/apply.rs` | hqf 快速修复部署 | `apply()` |
| `src/devtools.rs` | 设备侧只读取证（截图 / UI dump / hilog） | — |
| `src/sign_local.rs` | 本地签名链（开发板/模拟器） | — |
| `src/agc.rs` + `src/agc_client.rs` | AGC 证书/设备/登录 | `list_certs()`, `list_devices()`, `whoami()` |

### 数据流

```
hvolt.toml → assemble → 标准 HarmonyOS 布局 → hvigorw → .hap → sign → push → device
                               ↗                         ↗
                         SDK (list/download/import)  JDK + Node + hvigorw + ohpm
```

## 配置系统

### `hvolt.toml`（项目级）

```toml
[app]
bundle = "com.example.app"          # 必填
name = "my-app"                      # 必填
version = "1.0.0"                    # 必填
vendor = "example"                   # 必填
icon = "$media:layered_image"        # 必填
sdk_version = "26.0.0"               # 兼容 SDK 版本
compile_sdk_version = "26.0.0"       # 编译 SDK 版本
members = ["src"]                    # 模块目录列表

[app.signing]                        # 可选，签名配置
store_file = "signing/debug/debug.p12"
store_password = "..."
key_alias = "debugKey"
key_password = "..."
certpath = "signing/debug/debug.cer"
profile = "signing/debug/debug.p7b"
sign_alg = "SHA256withECDSA"

[dependencies]                       # 模块依赖
# type = "native" → C++/NAPI 原生模块
```

### `hvolt.toml`（模块级，常用字段）

```toml
[module]
name = "entry"                       # 模块名；build-profile 与 module.json5 都用它（≠ 目录名）
routes = ["pages/Index"]             # → main_pages.json
entry = "EntryAbility.ets"           # 入口 UIAbility
device_types = ["phone", "tablet"]
obfuscation = true                   # → build-profile release 的 ruleOptions.enable
obfuscation_rules = "obfuscation-rules.txt"  # 自定义规则；缺省自动发现同名文件

# 入口 Ability 的外观覆盖（缺省：应用图标 / auto_rotation）
icon = "$media:startIcon"
start_window_icon = "$media:startIcon"
orientation = "auto_rotation_unspecified"

# 安装语义覆盖（缺省按 bundle_type 推：atomicService → 免安装）
delivery_with_install = true
installation_free = true
```

`[module] icon` / `start_window_icon` / `orientation` 只作用于 `entry` 指的入口 Ability；
`[[module.abilities]]` 的 UIAbility 用各自的 `orientation`。

### 缓存目录

ohpm 的 store 在 `$HOME/.ohpm`，hvigor 缓存在 `$HOME/.hvigor`。受限环境（容器只读 home、
CI、沙箱）里第一次构建会以 `EPERM` 失败，且报错看起来像工程问题。
用 `hvolt build --cache-dir <dir>` 或环境变量 `HVOLT_CACHE_HOME=<dir>` 把它们移出去；
hvolt 自己的 `~/.hvolt`（工具、SDK）不受影响。

### 原生代码

两条路，别混：

| 形态 | 位置 | assemble 行为 |
|---|---|---|
| 独立 native 模块 | `<mylib>/hvolt.toml` + `CMakeLists.txt`，并在 `[dependencies]` 声明 | 拷 cpp、写 types 桥、加 `externalNativeOptions` |
| 模块内联 cmake | `<member>/cpp/CMakeLists.txt`（+ `<member>/libs/<abi>/*.so`） | 递归拷到 `src/main/cpp`、`src/main/libs`，写 `externalNativeOptions`（abiFilters 取 `[module] abi_filters`） |

**预编译输入放源树**：`<member>/cpp/prebuilt/` 之类的位置，并在 CMakeLists 里引用；
不要放进 `build/`——全量 assemble 会清空该目录。CMakeLists 中若有**越出模块**的相对路径
（`../../...`），`hvolt import` 会以「可能」措辞给出 gap：扁平布局不保证这些路径存在，
hvolt 也不改写 CMake 文件。

`cpp/`、`libs/` 永远不会被镜像进 `ets/`——曾经会，结果是 `.cpp` 与 `CMakeLists.txt` 明码打进 HAP。

### 目录与资源的归属

| 路径 | 翻译到 | 说明 |
|---|---|---|
| `<member>/resources/**` | 模块 `src/main/resources/` | 多语言、dark、resfile 放这里 |
| `resources/**`（根） | 同上（**与模块资源合并**，模块优先） | 应用图标等少量共享资源 |
| `app-resources/**` | `AppScope/resources/base/**` | 应用级 element/media（`$r('app.string.*')` 的归属） |

### `~/.hvolt/config.json`（用户级）

```json
{
  "sdk_path": "/path/to/sdk",
  "node_home": "/path/to/node",
  "java_home": "/path/to/jdk"
}
```

### SDK 布局

```
~/.hvolt/sdk/
├── v26.0.0.23/
│   ├── sdk-pkg.json          # 元数据
│   ├── openharmony/          # OpenHarmony SDK 组件
│   │   ├── ets/
│   │   ├── js/
│   │   ├── native/
│   │   ├── previewer/
│   │   └── toolchains/
│   │       └── hdc           # 设备连接工具
│   └── hms/                  # HMS 组件
└── default → v26.0.0.23      # 当前版本符号链接
```

**同一 SDK 有四种形态，不要混用**（混用会导致构建能过、扫描或校验失败）：

| 形态 | 路径 | 谁在用 |
|---|---|---|
| store | `~/.hvolt/sdk/{default→v<ver>, v<ver>/openharmony/<组件>/oh-uni-package.json}` | hvolt 管理/下载 |
| 构建视图 | `~/.hvolt/sdk-ohos/<ver>/{ets,js,native,previewer,toolchains}` | hvigorw（`OHOS_BASE_SDK_HOME`） |
| DevEco 布局 | `<DevEco>/sdk/default/{openharmony,hms}/{组件}` | 装了 IDE 的机器 |
| **lint 桥接层** | `~/.hvolt/lint/sdkroot/<platformVersion>/<组件>`（链接到 store） | **codelinter 校验器只认这个形态** |

## 代码约定与验收闸

改 `src/**` 的规范本体在随本二进制内置的 `clean-code` 文档。三条硬闸（缺一不提交）：

```bash
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test
```

要点速览：错误处理到 CLI 边界统一 `Result<(), String>` 且不吞 `Result`；参数 > 7 用
`#[allow(clippy::too_many_arguments)]` + 一行理由；返回元组 ≥3 元素起 `type` 别名；
不留 "legacy" 兼容壳；`mod tests` 放文件末尾；全 crate 唯一 `unsafe` 是 `main.rs` 的 SIGPIPE 复位。

## CLI 命令参考

### 项目命令

| 命令 | 说明 |
|---|---|
| `hvolt init <name>` | 生成新项目脚手架 |
| `hvolt import [--from <dir>] [--out <dir>] [--dry-run]` | DevEco 标准布局 → 扁平工程；同时列出扁平布局表达不了的项 |
| `hvolt assemble` | 翻译 → 标准 HarmonyOS 布局到 `build/` |
| `hvolt build [--cache-dir <dir>]` | assemble + ohpm install + hvigorw assembleApp + sign；`--cache-dir` 把 ohpm/hvigor 缓存移出 `$HOME`（沙箱/CI） |
| `hvolt lsp --stdio` | 语言服务器（LSP over stdio），供编辑器/agent 集成 |
| `hvolt skill list \| show <id>` | 列出/打印内嵌文档（hvolt-skill、clean-code、harmony-device-flow） |
| `hvolt run` | 全流程（build + push + launch） |
| `hvolt clean` | 清理构建产物 |
| `hvolt doctor` | 环境/SDK/工具/设备/凭据一致性预检（exit≠0 表示存在 ✗ 项） |
| `hvolt log` | 设备 hilog：`--level/--bundle/-T tag/-D domain/-k 关键词/--crash/--follow/--tail` |
| `hvolt file send·recv` | 设备文件传输（完成判据 `FileTransfer finish`） |
| `hvolt uninstall [bundle]` | 卸载（bundle 缺省取 hvolt.toml） |

### SDK 命令

| 命令 | 说明 |
|---|---|
| `hvolt sdk info` | 显示 SDK 路径 + 工具检测状态 |
| `hvolt sdk list-remote` | 列出远程可下载版本 |
| `hvolt sdk download <ver>` | 下载 SDK（支持 `--components` 过滤） |
| `hvolt sdk import <path>` | 导入已有 SDK 到 `~/.hvolt/sdk/` |
| `hvolt sdk list` | 列出本地已安装版本 |
| `hvolt sdk use <ver>` | 切换当前版本 |
| `hvolt sdk remove <ver>` | 删除版本 |
| `hvolt sdk set <path>` | 设置 SDK 路径（`--global` 全局） |

### 工具命令

| 命令 | 说明 |
|---|---|
| `hvolt tools list` | 列出已安装工具 |
| `hvolt tools install hvigor` | 安装 hvigorw（npm） |
| `hvolt tools install ohpm` | 安装 ohpm（镜像 zip；`--version` 任意镜像版本，>200MB 先报体积） |
| `hvolt tools list-remote` | 实查 registry/镜像可用版本与体积 |

### 配置命令

| 命令 | 说明 |
|---|---|
| `hvolt config get` | 显示全部配置 |
| `hvolt config set java_home <path>` | 设置 JDK 路径 |
| `hvolt config set node_home <path>` | 设置 Node.js 路径 |
| `hvolt config set sdk_path <path>` | 设置 SDK 路径 |
| `hvolt config unset <key>` | 删除配置项 |

### 检查与扫描

| 命令 | 说明 |
|---|---|
| `hvolt lint [--all-rules] [--json] [--fail-on <1..3>] [--max-defects <N>]` | 官方 Code Linter（codelinter）扫描翻译树；**无需装 DevEco**，机制见下节 |
| `hvolt check [--size-limit 2M]` | 包侧质量闸（官方 app_check_tool） |
| `hvolt hap info` | HAP 内省（bundle/版本/权限） |

### 签名 / 部署 / 设备

| 命令 | 说明 |
|---|---|
| `hvolt sign` | AGC 签名：CSR→证书→登记设备→profile（需先 `hvolt agc login`） |
| `hvolt sign --local` | 官方公开测试链本地签名（**仅开发板/模拟器**；零售机装不上） |
| `hvolt apply` | hqf 增量补丁部署（要求已装同身份包） |
| `hvolt push [-d serial]` | 推送 newest `output/*-signed.hap` 并安装 |
| `hvolt screenshot -o x.jpeg [-d serial]` | 设备截图（设备侧输出 jpeg） |
| `hvolt ui dump` | ArkUI 布局树 dump（uitest dumpLayout） |
| `hvolt device list \| connect <host:port> \| remote` | 设备列表 / TCP 连接（tconn）/ 登记跨机安装目标 |
| `hvolt emulator list\|start\|stop\|create\|delete\|images\|license` | 模拟器与系统镜像（**二进制与镜像随 DevEco Studio 分发**，仅 Windows/macOS；Linux 无官方二进制） |
| `hvolt tools install codelinter --from <dir>` | 安装扫描 payload（目录来源） |

### AGC

| 命令 | 说明 |
|---|---|
| `hvolt agc login` | 浏览器授权登录（token 落 `~/.hvolt/auth.json`；回调页需在**目标机**打开） |
| `hvolt agc certs \| devices \| whoami` | 证书列表/删除、已登记设备、当前账号 |

## 代码扫描 `hvolt lint`（官方 codelinter，实测约束）

> **DevEco 依赖边界（全项目只有两处）**：①`hvolt lint` 需要 DevEco 的 codelinter payload
> ——一次性从有 DevEco 的机器复制（`--from <目录>`），之后本机**不再需要** DevEco；
> ②`hvolt emulator` 需要 DevEco 的模拟器二进制与镜像。其余命令（init/assemble/build/
> sign/push/run/check/hap/log/screenshot/ui/file/uninstall/device/sdk/tools/config/doctor/agc）
> 均不需要 DevEco：SDK 与工具由 hvolt 自己下载，hdc 取自 SDK `toolchains/`。

华为不公开这些；以下每条都是对 DevEco 26.x 实测所得。

### payload：来源与安装

- **官方渠道实测**（据此断定取包不会有自动通道）：npm 六个候选全 404；官方 SDK 组件清单无 linter；`repo.harmonyos.com/deveco-studio/` 是 Marketplace SPA、`mirrors.huaweicloud.com/deveco-studio/` 是镜像门户 SPA（都不是文件库）；`contentcenter-vali-drcn.dbankcdn.cn` 根路径 **403**（只认签名时效直链）。
- **hvolt 不提供从安装包抽取 payload 的功能**（用户明确否决）：`--from <目录>` 是唯一入口，取包是用户自己的事。
- 官方**没有 npm 包**（`@ohos/codelinter`、`code-linter`、`linter`、`@performance/codelinter`、`arkperfcheck`、`performanceagent` 六个候选在 registry.npmjs.org 与 repo.harmonyos.com/npm **全 404**），**SDK 组件清单里也没有 linter**。payload 只能取自 DevEco 安装目录或其安装包。
- 实测体积 **200MB**（`typescript` 61MB 必需；顶层 `lib/` 13MB 是 IDE 侧 jar，可裁 → 187MB）。
- 安装：`hvolt tools install codelinter --from <DevEco>/plugins/codelinter` → `~/.hvolt/tools/codelinter`；发现顺序：env `HVOLT_CODELINTER` → 该目录 → DevEco 路径。
- payload 里 `performanceAgent/cppaudit`、`hpaudit` 是**平台原生**（Mach-O/ELF/PE），跨平台不可用；ArkTS/TS 引擎是纯 JS，可跨平台。

### 引擎调用约束

- `--sdkNumberVersion` 必须**点分形式**（`26.0.0`；传 `26` 会被 ApiVersion 拒绝）。
- `--dir` 收 JSON 数组；引擎对**任何名为 `build` 的路径段有内置忽略**，所以 hvolt 把翻译树复制到 `~/.hvolt/lint/tree/<slug>-<指纹>` 再扫，不直接扫 `build/`。
- 引擎**总是 exit 0**，判据只能看 `defects` 数组与引擎日志。

### 校验器的硬要求（无 DevEco 时的唯一闸门）

- 只有 **OpenHarmony** 工程会读 `<--project>/local.properties`；**HarmonyOS 工程走 `getHosSdk`、根本不读**（同一份 payload 在装了 DevEco 的 mac 上能用，正是因为那台工程是 `runtimeOS: HarmonyOS`）。
- `sdk.dir` 是它扫描的根，组件**只被接受**当目录恰为 **`<sdkRoot>/<platformVersion>/<组件>`** 且该组件目录含 `oh-uni-package.json`。store 的组件在 `<store>/<版本>/openharmony/<组件>`，位置不符 → 报 `The OpenHarmony SDK is not found` → **eslintAgent 与 arkPerfCheck 整体跳过**。
- 值**必须用正斜杠**：properties 解析会吃掉转义（`\v` 之类），反斜杠路径必然失败。
- hvolt 已自动桥接：把 store 链接成 `~/.hvolt/lint/sdkroot/<platformVersion>/<组件>`（unix symlink；Windows 先试 `symlink_dir`、失败回落 `mklink /J`）并写入 `sdk.dir=`。
- **假绿闸门**：引擎被跳过时报错退出并引引擎日志原文，不再印 `✓ Lint clean: 0 files`。

### 用法与判据（CI 面）

- 默认走官方 recommended 规则集；`--all-rules` 才带 `plugin:@typescript-eslint/recommended` 全量规则（实测默认集会漏掉大量问题——本项目脚手架下默认集 3 文件 0 缺陷，全量集 54 缺陷/其中 5 个 sev1）。
- **`--json`**：stdout 只出一份 JSON（`files/total/qualifying/passed/error/defects[]`），失败理由走 stderr，退出码仍非零 → CI 可直接吃。
- **`--fail-on <1..3>`**（默认 1，1 最严重）与 **`--max-defects <N>`**（默认 0）共同决定退出码；实测：`--max-defects 999` → rc=0，`--fail-on 1` 遇 5 个 sev1 → rc=1。
- **`--fix` 尚不可用**，会给明确拒绝：codelinter 只改**它工程根内**的文件，而 hvolt 的源码在工程根外（根是扁平的 `hvolt.toml`+`src/`，`build-profile.json5` 在 `build/` 里），翻译镜像 `build/<member>/src/main/ets/<角色>/` 是翻译器**造出来的结构**、路径上无法反推回源码。要做得成需在 assemble 时记账 source↔mirror 映射。

## SdkEnv 结构体

传递给构建流程的环境快照：

```rust
pub struct SdkEnv {
    pub ohos_sdk: PathBuf,     // SDK 根目录（~/.hvolt/sdk 或配置路径）
    pub hvigorw: PathBuf,      // hvigorw 二进制
    pub ohpm: PathBuf,         // ohpm 二进制
    pub node_home: PathBuf,    // Node.js 家目录
    pub java_home: PathBuf,    // JDK 家目录
    pub hdc: Option<PathBuf>,  // hdc 工具（SDK toolchains 或 PATH）
    pub emulator: Option<PathBuf>,  // 模拟器
}
```

### 发现优先级

1. `hvolt config` 中的配置
2. 环境变量（`NODE_HOME`, `JAVA_HOME`）
3. `PATH` 查找

## 已知限制

### API
- `getSdkList` API 对 `linux/arm64` 返回空列表，`linux/x86_64` 报参数异常
- 历史注记：早期代码曾对 linux 也发 `"darwin"`（osArch 传错值导致的误判）；**已证伪并修正** — 现为 `("linux","x86_64") => ("linux","x64")`，linux/x64 toolchains 是 ELF 构建（实测，见 src/sdk.rs 注释）

### 构建
- SDK 中 `restool`、`hdc` 等工具是**平台原生二进制**（Mach-O / ELF）
- Linux Docker 内无法运行 macOS 版本的 SDK toolchains
- ohpm 需要通过 `.ohpmrc` 配置 proxy（环境变量 `http_proxy` 无效）

### lint
- 副本树同步**不跟随链接**（已修：实测 Windows 上源 177 文件 → 副本 178 文件，此前跟随 junction 会到 18337）。
- `--fix` 未实现（见上节判据里的实测原因）。
- `doctor` 的 `[lint]` 段只做**警告**（payload / sdk bridge 是否就绪），不把缺 DevEco 的机器算成问题。
- 规则集与引擎清单跟 IDE 版本绑定，换 DevEco 版本需重取 payload。
- 曾把"引擎跳过"归因于 **node 版本下限**，**已证伪并撤回**：node 20/22 与 24/26 表现相同，真因是 SDK 形态。

### doctor 的 device_types 判据
- 可用集合 = `modulecheck/module.json` 的 schema 合法集 **∩** 本 SDK `device-define` 实际存在的集合（hvigor 的门更窄）。实测：`phone` 不在 OpenHarmony schema；`car` 在本 SDK 无 `device-define`。
- 只做只读排查；设备侧只允许装/启停应用与读日志，不代用户注入输入。

## 开发指南

```bash
# 构建
cargo build
cargo build --release

# 运行
cargo run -- sdk list-remote
cargo run -- sdk info
cargo run -- config get
cargo run -- build  # 在 hvolt 项目目录中执行

# 测试
cargo test

# debug（查看完整 stderr）
RUST_BACKTRACE=1 cargo run -- build 2>&1
```

### 添加新命令

1. 在 `src/cli.rs` 的 `Command` enum 中添加变体 + 参数
2. 在 `src/main.rs` 的 `match` 中添加处理分支
3. 实现业务逻辑
4. `cargo build` 验证

### 模块翻译

`assemble()` 流程：

```
read hvolt.toml → parse config → discover modules → translate each module:
  1. generate build-profile.json5（含 externalNativeOptions）
  2. generate module.json5（ability/extension config）
  3. generate oh-package.json5（依赖声明）
  4. mirror source files（ETS/JS/C++ → build/.../ets/）
  5. copy resources（media/string/color → AppScope/module resources）
  6. generate root build-profile.json5 + app.json5 + hvigor config
```

## 构建与签名验收纪律

（方法论来源：OpenHarmony 系统应用签名实践参考；原则：**验证最终产物，不验证中间声明**）

### 验签验的是包，不是日志

1. 签名成功日志 ≠ 交付物正确。**验最终 signed HAP**：
   `java -jar hap-sign-tool.jar verify-app -inFile <signed.hap> -outCertChain <cer> -outProfile <p7b>`
   再 `verify-profile -inFile <p7b> -outFile <result.json>`，核对 `verifiedPassed: true`。
2. 从 HAP 内解出 `module.json`，交叉核对四字段与构建目标一致：
   `bundleName`、Profile 的 `bundle-info.bundle-name`、`apl`、`app-feature`；
   **`validity` 按 UTC 秒读**（模板沿用旧值即过期身份，签名工具不会替你把关）。
3. 记录 **HAP 的 SHA-256**：确认安装/交付的是同一份文件（hvolt build 后读盘 `md5sum`/`shasum` 即此纪律的轻量版）。

### 缓存检查（幂等重跑的前提）

生成文件存在 ≠ 反映当前配置。`build/`、`output/`、签名中间产物（profile p7b）都按
**输入 mtime** 判断有效性：模板/证书/配置更新后必须重生成；失败残留要清生成物
（源材料与证书不动）。hvolt 的 download 重入收敛、build 前 `rm -rf build` 冷跑都属此条。

### 设备链路的成功是分层的

- `hdc` 传输成功 ≠ 远端 `aa start` 成功：两层结果分别断言（hvolt push/run 输出对应两行）。
- 安装失败归因顺序：**签名信任 → 包名/更新身份 → Profile 时效 → 设备约束**；
  运行时权限失败才对应 API/模块声明/ACL。修复发生在真实故障层，不用反复重签掩盖功能问题。
- `aa start` 只对导出的 UIAbility 成立；非导出/UIExtension 组件沿宿主入口验收。

### 验签的诚实边界

OpenHarmony 公共测试证书链只对**预置该 CA 的设备**（源编译 ROM/开发板/emulator）成立；
零售 HarmonyOS 设备需 AGC 签发身份。两种身份域不可互替——`hvolt sign`（AGC）与
`hvolt sign --local`（OpenHarmony localSign）分别对应这两个域。
