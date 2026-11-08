# hvolt 技能 — HarmonyOS Community Build Tool

## 项目概述

hvolt（HarmonyOS Volt）是一个社区构建工具，将扁平化项目结构转换成 HarmonyOS 标准工程布局。

语言：Rust（edition 2021）
二进制：`hvolt`
仓库：`better-harmony-app-build`

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

配置格式的权威规范见 **`docs/agents/hvolt-toml.md`**：字段全表、`[dependencies]` 表项语法、
`[registries]`、版本范围、lock 文件与 `hvolt dep sync`。改配置或加字段前先读它。

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
| `src/support/cli.rs` | CLI 定义（clap derive） | `Args`, `Command` enum |
| `src/sdk/manage.rs` | SDK 发现/管理/下载 | `detect()`, `list_remote()`, `download_sdk()`, `import_sdk()` |
| `src/build/deps/pipeline.rs` | 构建流程编排（原生链） | `native_build()`, `build_and_sign()`, `run()` |
| `src/sign/package.rs` | 远程签名（AGC API） | `sign()`, `sign_remote()` |
| `src/sign/native_sign.rs` | 本地签名（进程内，无 java） | `sign_package()` |
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

> hvolt 不依赖任何外部构建器 / 包管理器 / JDK / 设备通道二进制：ArkTS 编译、打包、签名、设备通信都是本仓实现。
> 唯一保留的外部依赖是 SDK 里的**编译器**（`es2abc`、`restool` 等官方二进制）。

### 数据流

```
hvolt.toml → build-native → assemble-hap(资源) → assemble-ets(ArkTS→abc) → pack-hap → sign → install → device
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
```

`[module] icon` / `start_window_icon` / `orientation` 只作用于 `entry` 指的入口 Ability；
`[[module.abilities]]` 的 UIAbility 用各自的 `orientation`。

---|---|---|
| 独立 native 模块 | `<mylib>/hvolt.toml` + `CMakeLists.txt`，并在 `[dependencies]` 声明 | 拷 cpp、写 types 桥、加 `externalNativeOptions` |
| 模块内联 cmake | `<member>/cpp/CMakeLists.txt`（+ `<member>/libs/<abi>/*.so`） | 递归拷到 `src/main/cpp`、`src/main/libs`，写 `externalNativeOptions`（abiFilters 取 `[module] abi_filters`） |

**预编译输入放源树**：`<member>/cpp/prebuilt/` 之类的位置，并在 CMakeLists 里引用；
不要放进 `build/`——全量 assemble 会清空该目录。CMakeLists 中若有**越出模块**的相对路径
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

改 `src/**` 的规范本体在 `docs/agents/clean-code.md`。三条硬性检查（缺一不提交）：

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
| `hvolt migrate [--from <dir>] [--out <dir>] [--dry-run]` | DevEco 工程 → hvolt 布局；同时列出表达不了的项 |
| `hvolt build` | 原生链构建：编译 ArkTS → 打包 HAP → 签名（不调用外部构建器/包管理器/JDK） |
| `hvolt run` | 推送并启动当前包；配了 `[app.signing]` 时先 build + sign |
| `hvolt clean` | 清理构建产物 |
| `hvolt doctor` | 环境/SDK/工具/设备/凭据一致性预检（exit≠0 表示存在 ✗ 项） |
| `hvolt log` | 设备 hilog：`--level/--bundle/-T tag/-D domain/-k 关键词/--crash/--follow/--tail` |
| `hvolt file send·recv` | 设备文件传输（原生设备协议，双向，字节数与 sha256 可校验） |
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
    pub ohos_sdk: PathBuf,          // SDK 根目录（~/.hvolt/sdk 或配置路径）
    pub node_home: PathBuf,         // Node.js 家目录（LSP/模拟器包装用）
    pub emulator: Option<PathBuf>,  // 模拟器
}
```

### 发现优先级

1. `hvolt config` 中的配置
2. 环境变量（`NODE_HOME`）
3. `PATH` 查找

## 已知限制

### API
- `getSdkList` API 对 `linux/arm64` 返回空列表，`linux/x86_64` 报参数异常
- 历史注记：早期代码曾对 linux 也发 `"darwin"`（osArch 传错值导致的误判）；**已证伪并修正** — 现为 `("linux","x86_64") => ("linux","x64")`，linux/x64 toolchains 是 ELF 构建（2026-10-09 服务器实测，见 src/sdk/manage.rs 注释）

### 构建
- SDK 中 `restool`、`es2abc` 等编译器是**平台原生二进制**（Mach-O / ELF）
- Linux Docker 内无法运行 macOS 版本的 SDK toolchains

### lint
- 副本树同步**不跟随链接**（已修：实测 Windows 上源 177 文件 → 副本 178 文件，此前跟随 junction 会到 18337）。
- `--fix` 未实现（见上节判据里的实测原因）。
- `doctor` 的 `[lint]` 段只做**警告**（payload / sdk bridge 是否就绪），不把缺 DevEco 的机器算成问题。
- 规则集与引擎清单跟 IDE 版本绑定，换 DevEco 版本需重取 payload。
- 曾把"引擎跳过"归因于 **node 版本下限**，**已证伪并撤回**：node 20/22 与 24/26 表现相同，真因是 SDK 形态。

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

### 模块翻译

`assemble()` 流程：

```
read hvolt.toml → parse config → discover modules → translate each module:
  1. generate build-profile.json5（含 externalNativeOptions）
  2. generate module.json5（ability/extension config）
  3. generate oh-package.json5（依赖声明）
  4. mirror source files（ETS/JS/C++ → build/.../ets/）
  5. copy resources（media/string/color → AppScope/module resources）
  6. pack the module into a HAP and sign it
```

## 构建与签名验收纪律

（方法论来源：OpenHarmony 系统应用签名实践参考；原则：**验证最终产物，不验证中间声明**）

### 验签验的是包，不是日志

1. 签名成功日志 ≠ 交付物正确。**验最终 signed HAP**：
   `hvolt sign` 签完用内置校验（`ApplicationVerifier` + `ProfileVerifier`）自证，输出
   `✓ verified final package` 与 SHA-256；要外部复核时再用官方 jar 的 `verify-app` /
   `verify-profile`，核对 `verifiedPassed: true`。
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
