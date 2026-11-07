# 清除外部工具链依赖 — 待办

判定规则（用户 2026-11-04 裁定）：连探测与字段一起拔，文档中的依赖描述也要移除。
允许继续依赖 DevEco SDK 里的**编译器**（es2abc / restool），不允许依赖其工具链
（设备通道工具 / hvigorw / hvigor / ohpm / java）。

## 已完成

- `SdkEnv` 删除 `hvigorw` / `ohpm` / `java_home` 与设备通道路径字段；`ToolPaths` 收为二元
- `sdk::detect` 不再要求 hvigorw / ohpm 存在（原先 `ok_or_else` 会直接失败）
- `doctor` 删除 java / hvigor / ohpm / lint 四段检查
- `hvolt lint` 子命令与 `src/lint.rs` 删除
- `hvolt tools` 子命令与 `src/tools.rs` 删除（无外部工具可装）
- `--cache-dir`（ohpm/hvigor 缓存）删除
- `hvolt assemble` 子命令与 `src/assemble.rs` 删除（标准 DevEco 布局生成）
- `src/translator/strategy.rs`、`src/translator_resources.rs` 删除；
  `translator/mod.rs` 只保留 `discover_modules` / `generate_main_pages`

## 待办

### 1. 自有 codelinter（替代 DevEco codelinter）

删掉 `hvolt lint` 后，hvolt 失去了静态规则集能力。需要一套自己的规则引擎，
不依赖 DevEco 的 codelinter payload（npm 与 SDK 都不带它）。

要求：
- 输入：扁平工程（`hvolt.toml` + `src/`），不产出标准布局
- 规则来源：ArkTS 严格模式约束（`arkts-no-*`）、资源引用一致性、
  `module.json5`/`main_pages.json` 与实际路由的一致性
- 判据：对 `examples/` 下的工程跑出可复现的缺陷数与位置，且与构建结果一致

### 2. keytool → 纯 Rust（去 java）

`hvolt sign`（AGC）当前用 java 的 `keytool` 生成 p12：
`agc-local/sign_remote.rs` 里 `find_java()` → `keytool::find_keytool()`。

要清掉 java，需要：
- 用 `rcgen` 或 `openssl` 直接生成 EC P-256 私钥 + 自签证书
- 打包成 PKCS#12（`.p12`），密码与别名与现有工作流兼容
- 判据：生成的 p12 能被 AGC 接受并成功签发证书（与 keytool 版本逐项对照）

### 3. 设备通道二进制残留 — 已完成（2026-11-04）

对设备通道二进制的 `spawn` 归零，旧的 `src/` 直连模块删除。全部改走原生设备协议：

- `file send` → 设备协议 `send_file`
- `file recv` → 设备协议 `recv_file`（新增 device→host 方向：
  `CMD_FILE_INIT` 让 daemon 当发送方 → 等 `CMD_FILE_CHECK` → 回 `CMD_FILE_BEGIN`
  → 收 `CMD_FILE_DATA`；CLOSE 帧要当噪声忽略）
- `uninstall` → `shell bm uninstall -n <bundle>`
- `log` → `shell hilog`（跟随模式改为轮询去重）
- `screenshot` / `ui` → `shell snapshot_display|uitest` + `recv_file`

实测读数：25B 与 120000B 回拉 sha256 与原件一致；卸载→确认→重装通过；
log 取到 38MB 全量；截图 217455 字节（1316x2832，真图为锁屏时钟）；布局 88308 字节。

### 4. 文档 — 已完成（2026-11-04）

改了 `README.md`、`docs/agents/{hvolt-skill,harmony-device-flow,clean-code}.md`、
`docs/published/{hvolt-skill,harmony-device-flow,clean-code}.md`（后者是 `include_str!` 的源）：

- 架构表按真实模块重写（删 assemble/build_hvigor/tools/lint 行，加设备协议/usb/native_sign/hap_pack）
- 数据流图改为原生链
- 命令表删 `assemble`、`tools install *`、`config set java_home`、`--cache-dir`
- 缓存目录段（ohpm/hvigor 缓存）整段删除
- 依赖表删 JDK / hvigorw / ohpm 三行
- DevEco 依赖边界段：只剩 emulator
- SdkEnv 结构体按新定义重写
- lint 章节（codelinter）整段删除
- 设备流命令全部改为 `hvolt push` / 设备 shell / `hvolt device connect`

残留检查：`grep -iE 'hvigor|ohpm|JDK|codelinter|tools install'` 命中数全部为 0，
仅剩两处「hvolt 不依赖 hvigor / ohpm / JDK / 设备通道二进制」的声明（有意保留）。
嵌入文档测试（含私有标识检查）全过。
