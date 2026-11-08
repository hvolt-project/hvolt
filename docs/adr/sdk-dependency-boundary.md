# ADR: SDK 依赖边界

状态：已裁决
日期：2026-11-08

## 背景

hvolt 的目标是让鸿蒙工程的构建不依赖 SDK 的构建工具链。restool 已经换掉：`res_ids`
复现它的 id 分配，`res_index_build` + `res_index_write` 复现 `resources.index` 的字节，
两者都在真实工程上对账到零分歧。剩下还有哪些 SDK 输入，此前没有一份完整清单，导致
"清除依赖面"没有终点，也无从判断某个依赖该不该动。

## 清点

完整清单与量化读数见 [`docs/reports/2026-11-06-ohos-sdk-dependency-surface.md`](../reports/2026-11-06-ohos-sdk-dependency-surface.md)，
2026-11-08 已按当前源码复测更新。生产路径剩下的 SDK 二进制是 `es2abc`（编译器）与 `llvm-strip`
（host 上未必有同目标的副本）；`restool`、`syscap_tool`、`cmake`、`ninja`、`ts2abc.js`、
`modulecheck/module.json` 都已不在生产路径上。

仍在生产路径读的 SDK 数据文件：`kit_configs/*.json`、`sysResource.js`、`components/*.json`。

## 裁决

1. **`es2abc` 是硬边界，不清除**。它产出 ArkCompiler 的 `.abc`，而 oxc 只做 JS/TS 解析与
   代码生成；要替换它等于重写 ArkCompiler 后端。项目已用 oxc 处理源码变形（`ets_transform`、
   `lower/*`），字节码生成这一步仍交给 SDK。
2. **三个数据文件不内嵌**。它们随 SDK 版本变化，而 hvolt 支持多版本并存与切换
   （`sdk/manage.rs` 的 `list` / `use` / `download` / `remove`，版本目录 `~/.hvolt/sdk/v<version>`）。
   内嵌等于把版本钉死，与用户选定的 SDK 版本失配，是正确性风险而不是依赖面缩小。
   正确做法是继续按当前版本从 SDK 读，读不到就报错。
3. **对账通道保留在测试侧**。restool 与 ts2abc.js 的可达性只服务对照，release 构建里没有
   到它们的路径。
4. **通用工具走 host，OHOS 特有件才取 SDK**。`cmake` / `ninja` 是通用工具，`native_compile.rs`
   的 `host_tool_or_sdk` 先查 PATH，SDK 副本只作回落。`llvm-strip`、`ohos.toolchain.cmake`、
   NDK 的 `libc++_shared.so` 是 OHOS 特有件，仍从 SDK 取。

## 后果

- "清除 SDK 依赖面"这条线在代码层面到此为止：剩余唯一的生产期 SDK 二进制是 `es2abc`。
- 新增 SDK 数据文件依赖时，按第 2 条处理——从 SDK 读，不内嵌，不缓存跨版本。
- 摆脱 `es2abc` 需要 ArkCompiler 后端，本仓库不具备该能力；这条边界由第 1 条裁决固定，不随依赖面清点进展变化。
