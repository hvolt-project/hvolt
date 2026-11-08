# hvolt 技能 — HarmonyOS Community Build Tool

## 项目概述

hvolt（HarmonyOS Volt）是一个社区构建工具，将扁平化项目结构转换成 HarmonyOS 标准工程布局。

语言：Rust（edition 2021）
二进制：`hvolt`
仓库：`hvolt-project/hvolt`（公开社区项目）

### 快速开始

从 DevEco 工程到设备上跑起来，五步：

```bash
hvolt doctor                       # 1. 环境自检：SDK、node、设备后端
hvolt migrate --dry-run --from .   # 2. 看它打算怎么迁（不落盘）
hvolt migrate --from .             # 3. 迁成扁平工程，输出目录在 <name>-flat/
cd <name>-flat && hvolt build      # 4. 编译打包；没配 [app.signing] 时只出 unsigned
hvolt sign --local                 # 5. 开发板/模拟器：本地签名（零售机必须走 AGC，见「签名」）
hvolt push                         #    装到设备（启动用 hvolt run）
```

从零建工程也可以（不需要先有 DevEco 工程）：

```bash
hvolt init myapp && cd myapp    # 生成最小可构建工程（resources/ 在项目根）
hvolt build && hvolt sign --local && hvolt push
```

两条常见断点：

- **没配签名**：`build` 仍会成功，产物是 `build/*-unsigned.hap`；`push`/`check` 会说明这一点，
  不会让你反复重跑 build。开发板用 `hvolt sign --local`。
- **设备没连上**：先 `hvolt device list` 看有没有设备，再 `hvolt push <addr>`。

### 扁平项目布局（hvolt 自己的工程格式）

hvolt 不只是构建工具，它**定义**工程格式：`hvolt.toml` 加源码与资源，编译与打包都在 hvolt 内完成，不生成 DevEco 工程。
权威示例：`examples/hello-world`（最小）与 `examples/hello-native`（含 C++/NAPI 原生模块）。

```
<app>/
├── hvolt.toml              # 应用级：[app] bundle/name/version/vendor/icon/sdk_version/members、[app.signing]、[dependencies]
├── src/                    # 一个模块（目录名即模块名）
│   ├── hvolt.toml          # 模块级：[module] name/routes/entry/router_map/forms/backup…
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
3. `types` 指向类型声明；`hvolt build` 按它解析 NAPI 类型并把编出的 `.so` 打进 HAP，
   并在模块 `oh-package.json5` 里加 `"<依赖名>.so": "file:./src/main/cpp/types/<依赖名>"`，
   同时写 `buildOption.externalNativeOptions.path = ./src/main/cpp/CMakeLists.txt`、`abiFilters = ["arm64-v8a"]`。

## 架构

### 核心模块

| 模块 | 职责 | 关键函数 |
|---|---|---|
| `src/main.rs` | 入口，命令分发 | `main()` |
| `src/support/cli.rs` | CLI 定义（clap derive） | `Args`, `Command` enum |
| `src/sdk/manage.rs` | SDK 发现/管理/下载 | `detect()`, `list_remote()`, `download_sdk()`, `import_sdk()` |
| `src/build/deps/pipeline.rs` | 构建流程编排（原生链） | `native_build()`, `build_and_sign()`, `run()` |
| `src/sign/backend.rs` | 远程签名（AGC API，随 feature 注入） | `sign_remote()` |
| `src/sign/sign_local.rs` | 本地签名（OpenHarmony localSign，`hvolt sign --local`） | `sign()` |
| `src/build/artifacts/hap_pack.rs` | HAP 打包（替代 app_packing_tool.jar） | `pack()` |
| `src/support/config.rs` | `hvolt.toml` 解析 | `AppConfig`, `ModuleConfig`, `NativeModuleConfig` |
| `src/translator/` | 扁平工程发现（模块/原生模块） | `discover_modules()`, `generate_main_pages()` |
| `src/project/scaffold.rs` | 项目脚手架生成 | `scaffold()` |
| `src/project/check.rs` | 包侧质量检查（直接读归档目录） | `check_package()` |
| `src/project/hap.rs` | HAP 内省（bundle/版本/权限） | `hap_info()` |
| `src/project/doctor.rs` | 环境/SDK/设备/凭据一致性预检 | `run()` |
| `src/device/registry.rs` | 设备枚举与管理 | `list()`, `connect()` |
| `src/device/devtools.rs` | 设备侧操作（日志/截图/布局/文件/卸载），走原生设备协议 | `log()`, `screenshot()`, `file_send()`, `file_recv()`, `uninstall()` |
| `src/device/usb.rs` | USB 传输层（nusb + USBHead 信封） | `UsbTransport::open()` |
| `src/sign/sign_local.rs` | 本地签名链（开发板/模拟器） | — |

> hvolt is self-contained: no external build tool, package manager, JDK, or device binary — ArkTS compilation, packing, signing and device I/O are all in this repo.
> 唯一保留的外部依赖是 SDK 里的**编译器**（`es2abc`、`restool` 等官方二进制）。

### 数据流

```
hvolt.toml → native → res → ets(ArkTS→abc) → pack → sign → install → device
                    ↘                ↘                     ↘
                  SDK: es2abc / restool / ArkTS 声明文件（只取编译器）
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

# 被别的模块 import 时用的入口文件（`Index.ets` → `Index`）。
# 缺省读模块自己的 oh-package.json5 `main`。
main = "Index.ets"

# 路由页：`NavPathStack.pushPathByName("…")` 的目标。
# 只写 name 就够——page 与 build_function 由构建扫源码推导（`@Builder export function`
# 的名字去掉 Builder 后缀、或文件名，与 name 忽略大小写相等即可）。推不出的才写全。
[[module.router_map]]
name = "route-address"
page = "components/More/Overwrite/Network.ets"
build_function = "NetworkRouterAddressBuilder"

# 桌面卡片 → 打包成 form_config.json
[[module.forms]]
name = "SwitchProxyWidget1x2"
display_name = "$string:widget_display_name"
description = "$string:widget_desc"
src = "widget/pages/SwitchProxyWidget1x2.ets"
default_dimension = "1*2"
support_dimensions = ["1*2"]

# 备份策略 → 打包成 backup_config.json
[module.backup]
allow_to_backup_restore = true
```

这四个字段（`router_map` / `main` / `forms` / `backup`）是 `hvolt migrate` 从
`route_map.json` / `oh-package.json5` / `form_config.json` / `backup_config.json` 折进来的，
迁移后工程里不再有那四个文件；直接手写工程时也只需要写这里。

`[module] icon` / `start_window_icon` / `orientation` 只作用于 `entry` 指的入口 Ability；
`[[module.abilities]]` 的 UIAbility 用各自的 `orientation`。

| 形态 | 源侧 | hvolt 做的 |
|---|---|---|
| 独立 native 模块 | `<mylib>/hvolt.toml` + `CMakeLists.txt`，并在 `[dependencies]` 声明 | 拷 cpp、写 types 桥、加 `externalNativeOptions` |
| 模块内联 cmake | `<member>/cpp/CMakeLists.txt`（+ `<member>/libs/<abi>/*.so`） | 递归拷到 `src/main/cpp`、`src/main/libs`，写 `externalNativeOptions`（abiFilters 取 `[module] abi_filters`） |

**预编译输入放源树**：`<member>/cpp/prebuilt/` 之类的位置，并在 CMakeLists 里引用；
不要放进 `build/`——`hvolt clean` 会清空该目录。CMakeLists 中若有**越出模块**的相对路径
（`../../...`），`hvolt migrate` 会以「可能」措辞给出 gap：扁平布局不保证这些路径存在，
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
  "device_target": "192.168.3.5:8710"
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
│   │   └── toolchains/       # es2abc / restool 等官方编译器
│   └── hms/                  # HMS 组件
└── default → v26.0.0.23      # 当前版本符号链接
```

**同一 SDK 有四种形态，不要混用**（混用会导致构建能过、扫描或校验失败）：

| 形态 | 路径 | 谁在用 |
|---|---|---|
| store | `~/.hvolt/sdk/{default→v<ver>, v<ver>/openharmony/<组件>/oh-uni-package.json}` | hvolt 管理/下载 |
| 构建视图 | `~/.hvolt/sdk-ohos/<ver>/{ets,js,native,previewer,toolchains}` | 原生编译链（只取编译器） |
| DevEco 布局 | `<DevEco>/sdk/default/{openharmony,hms}/{组件}` | 装了 IDE 的机器 |

## 代码约定与验收检查

改 `src/**` 的规范本体在随本二进制内置的 `clean-code` 文档。三条硬性检查（缺一不提交）：

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
| `hvolt migrate [--from <dir>] [--out <dir>] [--dry-run]` | DevEco 工程 → hvolt 布局（只留 `hvolt.toml`）；同时列出表达不了的项 |
| `hvolt build` | 编译（自建 ArkTS 链）、打包与签名（不依赖外部构建工具 / 包管理器 / JDK） |
| `hvolt lsp --stdio` | 语言服务器（LSP over stdio），供编辑器/agent 集成 |
| `hvolt skill list \| show <id>` | 列出/打印内嵌文档（hvolt-skill、clean-code、harmony-device-flow） |
| `hvolt run` | 推送并启动当前包；配了 `[app.signing]` 时先 build + sign |
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
| `hvolt check [PATH]` | 包侧质量门：重复文件、超大文件、后缀统计（缺省取最新 signed 包） |
| `hvolt hap info [PATH]` | 读出 HAP 的 module.json / 条目数 / 大小 |
| `hvolt clean` | 清掉 `build/` |
| `hvolt doctor` | 环境自检（SDK / node / 设备后端），有警告时 Summary 会分开报 |

### 配置命令

| 命令 | 说明 |
|---|---|
| `hvolt config get` | 显示全部配置 |
| `hvolt config set node_home <path>` | 设置 Node.js 路径 |
| `hvolt config set sdk_path <path>` | 设置 SDK 路径 |
| `hvolt config unset <key>` | 删除配置项 |

### 检查与扫描

| 命令 | 说明 |
|---|---|
| `hvolt check [--size-limit 2M]` | 包侧质量检查（直接读归档目录） |
| `hvolt hap info` | HAP 内省（bundle/版本/权限） |

### 签名 / 部署 / 设备

| 命令 | 说明 |
|---|---|
| `hvolt sign [HAP]` | AGC 签名：CSR→证书→登记设备→profile（需先 `hvolt agc login`）。给了 `HAP` 时签那个裸包：默认走 localSign 测试链，profile 绑**包内 bundleName**（缺失时 `--bundle` 补），结果留在包所在目录的 `<name>-signed.hap`（输入以 `-unsigned` 结尾则换成 `-signed`），签完由内置校验自证（`ApplicationVerifier` + `ProfileVerifier`）；`--p12/--profile/--cert` 三项同时给出时改用这些材料签 |
| `hvolt sign --local [HAP]` | 官方公开测试链本地签名（**仅开发板/模拟器**；零售机装不上）。给了 `HAP` 时签该裸包，行为同上；**已带签名的包会被拒绝**（按包内 `META-INF` 判，不看文件名） |
| `hvolt push [-d serial] [--hap FILE]` | 推送 newest `output/*-signed.hap` 并安装；`--hap` 指定任意位置的包文件，**未签名的包直接报错**而不是丢给设备 |
| `hvolt screenshot -o x.jpeg [-d serial]` | 设备截图（设备侧输出 jpeg） |
| `hvolt ui dump` | ArkUI 布局树 dump（uitest dumpLayout） |
| `hvolt device list \| connect <host:port> \| remote` | 设备列表 / TCP 连接（tconn）/ 登记跨机安装目标 |
| `hvolt emulator list\|start\|stop\|create\|delete\|images\|license` | 模拟器与系统镜像（**二进制与镜像随 DevEco Studio 分发**，仅 Windows/macOS；Linux 无官方二进制） |

### AGC

| 命令 | 说明 |
|---|---|
| `hvolt agc login` | 浏览器授权登录（token 落 `~/.hvolt/auth.json`；回调页需在**目标机**打开） |
| `hvolt agc certs \| devices \| whoami` | 证书列表/删除、已登记设备、当前账号 |

## SdkEnv 结构体

传递给构建流程的环境快照：

```rust
pub struct SdkEnv {
    pub ohos_sdk: PathBuf,          // SDK 根目录
    pub node_home: PathBuf,         // Node.js 家目录（LSP/模拟器包装用）
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
- 历史注记：早期代码曾对 linux 也发 `"darwin"`（osArch 传错值导致的误判）；**已证伪并修正** — 现为 `("linux","x86_64") => ("linux","x64")`，linux/x64 toolchains 是 ELF 构建（实测，见 src/sdk/manage.rs 注释）

### 构建
- SDK 中 `restool`、`es2abc` 等编译器是**平台原生二进制**（Mach-O / ELF）
- Linux Docker 内无法运行 macOS 版本的 SDK toolchains

### doctor 的 device_types 判据
- 可用集合 = `modulecheck/module.json` 的 schema 合法集 **∩** 本 SDK `device-define` 实际存在的集合。实测：`phone` 不在 OpenHarmony schema；`car` 在本 SDK 无 `device-define`。
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

1. 在 `src/support/cli.rs` 的 `Command` enum 中添加变体 + 参数
2. 在 `src/main.rs` 的 `match` 中添加处理分支
3. 实现业务逻辑
4. `cargo build` 验证

### 一次 `hvolt build` 做什么

```
read hvolt.toml → discover modules → for each module:
  native   : [dependencies] 的 path 指向的 CMakeLists.txt → libs/<abi>/*.so（SDK toolchain）
  res      : resources/ → module.json + 生成的 profile（route_map/form_config/backup_config）
  ets      : 源码 → 一份合并的 ets/modules.abc（自研 ArkTS 链，不经 hvigor）
  pack     : module.json + resources + abc + libs → <module>-unsigned.hap
  sign     : 有 [app.signing] 才签，否则只产出 unsigned
```

不生成 DevEco 工程：`build/` 下没有 `build-profile.json5` / `oh-package.json5` / `hvigorfile`，
所以 `hvigorw` 在那里没有可打开的工程。

## 构建与签名验收纪律

（方法论来源：OpenHarmony 系统应用签名实践参考；原则：**验证最终产物，不验证中间声明**）

### 验签验的是包，不是日志

1. 签名成功日志 ≠ 交付物正确。**验最终 signed HAP**：`hvolt sign` 签完用内置校验
   （`ApplicationVerifier` + `ProfileVerifier`）自证，输出 `✓ verified final package` 与 SHA-256；
   要外部复核时再用官方 jar 的 `verify-app` / `verify-profile`，核对 `verifiedPassed: true`。
2. 从 HAP 内解出 `module.json`，交叉核对四字段与构建目标一致：
   `bundleName`、Profile 的 `bundle-info.bundle-name`、`apl`、`app-feature`；
   **`validity` 按 UTC 秒读**（模板沿用旧值即过期身份，签名工具不会替你把关）。
3. 记录 **HAP 的 SHA-256**：确认安装/交付的是同一份文件（hvolt build 后读盘 `md5sum`/`shasum` 即此纪律的轻量版）。

### 缓存检查（幂等重跑的前提）

生成文件存在 ≠ 反映当前配置。`build/`、`output/`、签名中间产物（profile p7b）都按
**输入 mtime** 判断有效性：模板/证书/配置更新后必须重生成；失败残留要清生成物
（源材料与证书不动）。hvolt 的 download 重入保持幂等、build 前 `rm -rf build` 冷跑都属此条。

### 设备链路的成功是分层的

- 传输成功 ≠ 远端 `aa start` 成功：两层结果分别断言（hvolt push/run 输出对应两行）。
- 安装失败归因顺序：**签名信任 → 包名/更新身份 → Profile 时效 → 设备约束**；
  运行时权限失败才对应 API/模块声明/ACL。修复发生在真实故障层，不用反复重签掩盖功能问题。
- `aa start` 只对导出的 UIAbility 成立；非导出/UIExtension 组件沿宿主入口验收。

### 验签的诚实边界

OpenHarmony 公共测试证书链只对**预置该 CA 的设备**（源编译 ROM/开发板/emulator）成立；
零售 HarmonyOS 设备需 AGC 签发身份。两种身份域不可互替——`hvolt sign`（AGC）与
`hvolt sign --local`（OpenHarmony localSign）分别对应这两个域。
