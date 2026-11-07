# hvolt.toml 规范

hvolt 的唯一配置格式。一个应用由**一个根 `hvolt.toml`** 与**若干成员级 `hvolt.toml`** 组成，
每个第三方依赖包也以同样的格式描述自己。设计参照 Cargo：同一套字段定义在不同位置复用，
只有"能出现在哪些位置"不同。

- 根 `hvolt.toml`：[app]（应用身份）+ [dependencies]（全应用可见）+ [registries]（依赖源）
- 成员 `hvolt.toml`：[module]（模块身份）+ [dependencies]（本模块追加）
- 依赖包 `hvolt.toml`：[module]（包身份）+ 可选 [dependencies]（它自己的依赖）

## 基本规则

1. **一套字段定义**。`[app]` 与 `[module]` 共用同一组身份字段，语义一致；成员未写的字段从根继承。
2. **扁平布局**。项目里只出现 `hvolt.toml` 描述的源码树。`oh-package.json5`、`build-profile.json5`、
   `hvigorfile.ts`、`package.json` 这类 hvigor/ohpm 布局文件**不进入**扁平项目；依赖包的它们由
   `hvolt dep sync` 在转换阶段消化掉。
3. **依赖不是成员**。`members` 只列项目自己的源码模块；第三方包走 `[dependencies]`，不进 `members`。
4. **表格只写事实**。凡是能从源码树推导的（模块名、入口、资源、设备类型）都有缺省值，写出来的只能是覆盖。

## 目录布局

```
project/
├── hvolt.toml                  # 根：[app] + [dependencies] + [registries]
├── app-resources/              # 应用级资源
├── entry/                      # 成员
│   ├── hvolt.toml              # [module]
│   └── ...
├── proxy_core/                 # 成员
│   ├── hvolt.toml
│   ├── cpp/                    # 模块内联 cmake（可选）
│   └── libs/<abi>/*.so         # 预编译原生库（可选）
└── deps/                       # registry 里的 ArkTS 包扁平化后落在这里
    └── sandboxfinder/
        ├── hvolt.toml
        └── src/...
```

纯 JavaScript 包（如 `yaml`）**不落进项目**，编译时直接从 cache 引用。

## 根 `hvolt.toml`

```toml
[app]
bundle = "org.example.app"
name = "MyApp"
version = "1.0.0"
vendor = "example"
icon = "$media:layered_image"
sdk_version = "26.0.0"
compile_sdk_version = "26.0.0"
compatible_sdk_version = ""
runtime_os = "HarmonyOS"
members = ["entry", "proxy_core"]
products = ["default", "release"]
```

### [app] 字段

| 字段 | 类型 | 缺省 | 说明 |
|---|---|---|---|
| `bundle` | string | 必填 | bundleName |
| `name` | string | 必填 | 应用名（用于 module.json5 的 label 推导） |
| `version` | string | 必填 | versionName；`version_code` 缺省由它推导 |
| `version_code` | u32 | 0 = 推导 | 显式 versionCode |
| `build_version` | string | `"1"` | app.json5 的 buildVersion |
| `vendor` | string | `"example"` | vendor |
| `icon` | string | `$media:layered_image` | 应用图标 |
| `sdk_path` | string | 自动探测 | SDK 根目录 |
| `sdk_version` | string | `"26.0.0"` | 运行时 SDK |
| `compile_sdk_version` | string | 同 `sdk_version` | 编译 SDK |
| `target_sdk_version` | string | 同 compile | targetSdkVersion |
| `compatible_sdk_version` | string | 同 `sdk_version` | 最低兼容版本 |
| `bundle_type` | string | `"app"` | `app` / `atomicService` |
| `products` | array | `["default"]` | 构建产物配置名 |
| `runtime_os` | string | `"HarmonyOS"` | `HarmonyOS` / `OpenHarmony` |
| `members` | array | `[]` | 成员目录（相对项目根） |

### [app.signing.<product>]

```toml
[app.signing.default]
store_file = "signing/debug/debug.p12"
store_password = "..."
key_alias = "debugKey"
key_password = "..."
certpath = "signing/debug/debug.cer"
profile = "signing/debug/debug.p7b"
sign_alg = "SHA256withECDSA"
```

旧的单份写法 `[app.signing]` 等价于 `[app.signing.default]`。

## 成员 `hvolt.toml`

```toml
[module]
name = "entry"
routes = ["pages/Index"]
entry = "EntryAbility.ets"
device_types = ["phone", "tablet"]
```

### [module] 字段

| 字段 | 类型 | 缺省 | 说明 |
|---|---|---|---|
| `name` | string | 目录名 | 模块名；module.json5 与 build-profile 都用它 |
| `routes` | array | 自动发现 `pages/` | 页面路由，写 `main_pages.json` |
| `entry` | string | `EntryAbility.ets` | 入口 UIAbility 源文件 |
| `device_types` | array | `["phone","tablet"]` | deviceTypes |
| `obfuscation` | bool | `false` | release 的 `ruleOptions.enable` |
| `obfuscation_rules` | string | 自动发现同名文件 | 自定义混淆规则 |
| `icon` / `start_window_icon` / `orientation` | string | 从 app 继承 | 只作用于 `entry` 指的入口 Ability |
| `delivery_with_install` | bool | 按 bundle_type 推 | 安装语义 |
| `installation_free` | bool | 按 bundle_type 推 | 免安装 |
| `abi_filters` | array | `["arm64-v8a"]` | 原生库 ABI |

`[[module.abilities]]` 描述非入口的 Ability/ExtensionAbility（`type = ""` 为 UIAbility，
`type = "form"` 为表单扩展），字段与 `[module]` 的身份字段重叠，语义一致。

## [dependencies]

第三方包、原生模块、其他成员都写在这里。表项支持两种写法：

```toml
[dependencies]
# 1) 简写：只有版本，从 registry 拉
yaml = "^2.8.0"

# 2) 表：字段按需给
sandboxfinder = { version = "^1.0.5", registry = "default", entry = "Index.ets" }
mccharts = { path = "deps/mccharts", version = "2.8.9", type = "arkts" }
flclash = { path = "proxy_core/cpp", type = "native" }
```

### 表项字段

| 字段 | 类型 | 缺省 | 说明 |
|---|---|---|---|
| `path` | string | 无 | 本地包目录（相对项目根）。省略则从 registry 取 |
| `version` | string | 无（path 依赖可省） | semver 范围，见下 |
| `registry` | string | `"default"` | 取哪个 [registries] 源 |
| `type` | string | 按内容探测 | `arkts` / `js` / `native` |
| `entry` | string | 按惯例 | 包入口，`Index.ets`（arkts）或 package.json 的 main/module（js） |
| `alias` | string | 同 key | ArkTS 里的导入名；`alias` 用于包名与导入名不一致时 |
| `optional` | bool | `false` | 缺失时不报错，只记 gap |

`type` 的探测顺序：有 `src/main/ets/` 或 `Index.ets` → `arkts`；有 `package.json` 且无 arkts 根 →
`js`；有 `CMakeLists.txt` → `native`。

### 版本语法

支持完整 semver 范围：精确（`1.2.3`）、`^`、`~`、`>=`、`<`、`*`、`||` 组合。
解析结果写进 `hvolt.lock`；构建按 lock 走，`hvolt dep sync --locked` 在 lock 缺失或漂移时报错。

## [registries]

```toml
[registries.default]
url = "https://ohpm.openharmony.cn/ohpm/"

[registries.local]
url = "file:///opt/packages"
```

- 支持 `http(s)://` 与 `file://`（本地目录 / 内网私服）
- `default` 缺省指向公开 ohpm registry；全局配置 `~/.hvolt/config.json` 可覆盖默认值
- 依赖项用 `registry = "<name>"` 选源，省略用 `default`

## 依赖合并语义

与 hvigor 的实际行为一致：

- 根 `[dependencies]` 对**所有成员可见**（相当于 hoisting 到应用级）
- 成员 `[dependencies]` 在此之上**追加**；同名时**成员声明胜出**，只影响该成员
- 依赖包自己的 `[dependencies]` 只描述它自己的依赖，不向父级传播

同名冲突在 `hvolt dep sync` 阶段报错并列出双方来源，不做静默选择。

## 转换规则

依赖包是否需要"扁平化"由**包内布局**决定，不由来源决定：

| 包内形态 | 判定 | 处理 |
|---|---|---|
| `oh-package.json5` + `src/main/ets/` + `Index.ets` | ArkTS 包（hvigor 布局） | 扁平化：生成 `hvolt.toml`，映射入口与资源，剥掉 hvigor JSON，落到 `deps/<name>/` |
| 只有 `package.json` + ESM 源码树 | 纯 JS 包 | **不转换**，不落盘，编译时从 cache 直接引用 |
| `CMakeLists.txt` + `cpp/` | 原生模块 | 按 `type = "native"` 编译，声明 `lib<name>.so` |

扁平化产出的 `deps/<name>/` 是普通 hvolt 成员形态（`hvolt.toml` + 源码），但它**不是** `members`
的一员：它由依赖解析引入，`members` 仍然只列项目自己的源码。

## hvolt.lock

```toml
version = 1

[[package]]
name = "yaml"
version = "2.8.0"
registry = "https://ohpm.openharmony.cn/ohpm/"
hash = "sha256:..."
type = "js"

[[package]]
name = "sandboxfinder"
alias = "@cxy/sandboxfinder"
version = "1.0.5"
registry = "default"
hash = "sha256:..."
type = "arkts"
```

lock 记录：解析出的精确版本、来源 registry、内容哈希、类型。构建时按 lock 取包；
`hvolt dep sync --locked` 校验 lock 与声明是否一致。

## hvolt dep sync

```
hvolt dep sync              # 解析 → 下载 → 扁平化 → 写 deps/ 与 hvolt.lock
hvolt dep sync --locked     # 只按 lock 取，缺包或漂移即失败
hvolt dep sync --offline    # 只用 cache，不联网
```

`hvolt build` 会先跑一次 `dep sync`（行为等同不加参数）。cache 位于 `~/.hvolt/cache/`，
按 `<registry-host>/<name>/<version>/` 组织，命中即复用。

## 明确不做的事

- 不读扁平项目里的 `oh-package.json5` / `build-profile.json5` / `package.json` 作为**配置来源**：
  这些是 hvigor 布局，只在转换阶段作为输入被消化，转换后不留在项目里。
- 不因为依赖引入了包就把它塞进 `members`。
- 不在没有 lock 变化的情况下重新解析版本范围。

## 完整示例

根 `hvolt.toml`：

```toml
[app]
bundle = "org.xbgroup.clashboxLTS"
name = "ClashBox"
version = "1.9.9"
vendor = "xbgroup"
icon = "$media:layered_image"
sdk_version = "26.0.0"
compile_sdk_version = "26.0.0"
members = ["entry", "proxy_core", "xb_components"]

[app.signing.default]
store_file = "signing/debug/debug.p12"
store_password = "..."
key_alias = "debugKey"
key_password = "..."
certpath = "signing/debug/debug.cer"
profile = "signing/debug/debug.p7b"

[dependencies]
yaml = "^2.8.0"
mccharts = { version = "^2.8.9", entry = "index.ets" }
sandboxfinder = { version = "^1.0.5", alias = "@cxy/sandboxfinder", entry = "Index.ets" }
flclash = { path = "proxy_core/cpp", type = "native" }

[registries.default]
url = "https://ohpm.openharmony.cn/ohpm/"
```

成员 `entry/hvolt.toml`：

```toml
[module]
name = "entry"
routes = ["pages/Index"]
entry = "EntryAbility.ets"

[dependencies]
proxy_core = { path = "../proxy_core" }
xb_components = { path = "../xb_components" }
```
