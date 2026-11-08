# 三个 SDK 二进制替换为 Rust 实现的代价

对象：es2abc、restool、syscap_tool（cmake / ninja / llvm-strip 按用户裁定保留）。
样本：macOS 本机 SDK `v26.0.0.23`。采样时间 2026-11-06。

## 结论

| 工具 | 体积 | 建议 | 量级 | 主要依据 |
|---|---|---|---|---|
| syscap_tool | 445 KiB | 值得换 | **0.5–1 人日** | 编码格式已完全反推，官方 `-d` 可做逐字节对拍 |
| restool | 2.29 MiB | 可换，但要接受范围 | **3–6 人周** | 输入协议 13 个字段、语义面宽（id 分配 / 限定词 / 图标校验 / 预编译包合并） |
| es2abc | 9.37 MiB | 不建议换 | **6–12 人月（下限）** | 不是格式复刻，是重写编译器后端 + 需持续跟随 runtime 格式契约 |

## 一、syscap_tool：格式已完全确定

### 实测反推

工具自报 `syscap_tool v2.0.1`，错误日志暴露源码路径 `developtools/syscap_codec/src/{syscap_tool,context_tool,main}.c`。

穷举四种输入得到的输出结构：

| 输入 | 输出大小 | 首 6 字节 |
|---|---|---|
| `{"api_version":26,"syscap":[]}` | 6 | `00 9a 00 02 00 00` |
| `{"api_version":26,"syscap":[A]}` | 262 | `00 9a 00 02 01 00` |
| 同上，两项 | 518 | `00 9a 00 02 02 00` |
| `{"api_version":9,…}` | 262 | `00 89 00 02 01 00` |

由此得到的规则：

```
header: u16 (0x80 | api_version) | u16 0x0002 | u16le count
body:   count × 256 字节 —— syscap 名去掉 "SystemCapability." 前缀，NUL 填充
```

- `SystemCapability.NotReal.Thing` 编码后落在槽内的是 `NotReal.Thing`（实测 hexdump 可见）。
- api_version 参与编码：9 → `0x0089`，26 → `0x009a`，即 `0x80 | api`（两个样本）。
- 与仓库既有实测互相印证：[syscap.rs:1-7](src/build/artifacts/syscap.rs#L1-L7) 记录「206 syscaps、52 742 字节」；按公式 `6 + 206×256 = 52 742`，完全吻合。

### 工作项

1. 编码器（约 30 行）+ 解码器（用于自证）
2. 对拍：官方 `-d` 反解我产生的 `.sc`，与输入 JSON 逐字段一致
3. 穷举边界：api 1–100、count 0–32、名字长度 1–255、超长名截断行为

### 风险

`0x80 | api` 只有两个样本支撑，不能外推为"高位置位"。实现时必须以官方输出为唯一基准做穷举对拍，
不靠推导。名字超 255 字节的行为（截断还是拒绝）未测。

## 二、restool：格式不难，语义面宽

### 实测到的接口面

hvolt 构造的 `resConfig.json` 有 13 个字段（[res_compile.rs:196-238](src/build/artifacts/res_compile.rs#L196-L238)）：

`configPath`、`packageName`、`output`、`moduleNames`、`ResourceTable[.h,.ts]`、`applicationResource`、
`moduleResources`、`dependencies`、`iconCheck`、`ids`、`definedIds`、`definedSysIds`。

其中三项是真工作量，不是管道：

- `moduleNames` / `dependencies`：入口模块与依赖模块编译进同一张表，名字列表必须与 `dependencies` 对齐，
  否则 id 落到错误的包下（代码注释记录了这个失败模式）。
- `iconCheck`：默认为 true，走 PNG 图标校验（尺寸/格式/透明通道）。
- `definedIds` / `definedSysIds`：id 定义体系；hms 组件缺失时用空记录占位（[res_compile.rs:176-196](src/build/artifacts/res_compile.rs#L176-L196)）。

### 产物格式（都不复杂）

`ResourceTable.txt` 是逐行文本，实测样例只有两行：

```
string agcit_ui_linke_uicolorpager 0x01000000
```

`resources.index` 是 chunk 容器，实测 842 字节，首段明文 `Restool 6.1.0.003`，
随后 `KEYS` / `IDSS` 两个 chunk + 字符串池（`strings` 可直接读出资源名与路径）。
hvolt 已有一半解析器：[ets_resources.rs:38](src/build/compile/ets_resources.rs#L38) 的 `parse_table`（283 行文件）。

### 需要复刻的语义

| 项 | 说明 | 估时 |
|---|---|---|
| 资源发现 | `resources/{base,dark,…}/{element,media,profile,rawfile}` 与多语言/多密度限定词目录名解析 | 3–5 天 |
| 类型系统 | string/color/float/boolean/integer/plural/strarray/intarray/media/profile/pattern/font | 3–5 天 |
| id 分配 | type id 高位 + entry 递增 + 模块分区掩码（`-e startId`）+ 与 `dependencies` 对齐 | 2–3 天 |
| 引用解析 | `$r('app.string.x')`、`$media:`、layered_image.json、precompiled HAR 的 ResourceTable 复用 | 3–5 天 |
| 校验 | 重复 key、类型不符、悬空引用、资源名合法性 | 2–3 天 |
| 图标校验 | `iconCheck` 的 PNG 规则 | 1–2 天 |
| 序列化 | `ResourceTable.txt` + `resources.index`（chunk 格式已见）| 2–3 天 |
| 联调 | 对 `examples/` 与 50 个 fixtures 跑通并与官方输出对照 | 5–8 天 |

合计约 3–6 人周。可复用资产：`parse_table` 的解析侧、`res_compile.rs` 852 行的编排逻辑、
以及 OpenHarmony 的 `developtools_global_resource_tool` 作为语义参照（开源 C++）。

### 风险

- `resources.index` 的版本兼容：设备侧 ResourceManager 按版本读，自产文件的头版本号取什么值需要实测确认。
- 资源语义的边角（限定词组合、plural 复数规则、同 key 多来源合并顺序）只能靠对拍发现。

## 三、es2abc：不建议替换

### 为什么它不是"格式复刻"

输入协议确实窄（[abc_compile.rs:359-390](src/build/compile/abc_compile.rs#L359-L390)）：

```
es2abc --merge-abc @filesInfo.txt --npm-module-entry-list npmEntries.txt
        --output modules.abc --file-threads N
        --target-api-version 26 --enable-release-column --enable-callable-name
        [--debug-info] [--enable-abc-input] [--compile-context-info …]
```

`filesInfo.txt` 是每行一段的分号/竖线分隔记录，`npmEntries.txt` 是固定内容。这层不难。

难的是它内部：**TS → PANDA 字节码的完整编译器**。产物实测 magic 为 `PANDA`，
头部是一组 section 偏移表（样例 7 368 字节，含 13 项偏移），后面是 record/method/字面量/调试信息表。

### 需要复刻的东西

| 层 | 内容 | 可复用 |
|---|---|---|
| 解析 | TS 语法 | oxc（hvolt 已在用） |
| 语义 | 作用域/hoisting/闭包/模块系统/类/泛型擦除/装饰器 | 无 |
| 字节码 | PANDA 栈机 ISA、异常表、闭包变量捕获 | 无 |
| 数据表 | 常量池、字面量池、record 表、字符串池 | 无 |
| 调试 | debug-info、release column、callable name | 无 |
| 文件格式 | panda file 序列化（版本/偏移/校验） | 无 |
| runtime 契约 | 每条字节码在 libark 上的语义必须一致 | 无 |

前六项是"写一个编译器后端"，第七项是持续负担：SDK 升级可能改变 abc 格式版本或指令语义，
自研实现必须跟着追。这与 restool 的性质不同——restool 错了产不出合格资源，es2abc 错了产出的包在设备上直接崩。

### 量级判断（估算）

- es2abc 二进制 9.37 MiB（strip 后的 Mach-O），单一职责。
- hvolt 现有全部 Rust 代码 31 444 行，覆盖 lowering、打包、签名、设备通信、SDK 管理。
- 官方参考实现是 OpenHarmony 主仓（`arkcompiler_ets_frontend` + `runtime_core` 的 compiler 部分），
  C++ 十万行量级。

即便只做 hvolt 需要的子集（输入已是 lowering 后的 .ts，不含 ArkUI 语法），下限也在 **6–12 人月**，
且不含长期跟随 SDK 版本的维护。这个数字是估算，不是实测——真正的可证伪判据是：
能否让 `ark_disasm` 反汇编自产 `modules.abc` 与官方产物语义等价，并在设备上跑通。

## 四、被忽略的成本：golden 基准会失效

仓库有 50 个 `fixtures/*/golden/modules.abc` 与 50 个 `fixtures/*/golden/resources.index`，
作为与官方实现字节对齐的基准。换成自研实现后：

- 这 100 个基准全部失去"对齐官方"的意义，只能改成"与自研实现自洽"，证明力下降。
- 若改判据为语义等价（ark_disasm dump 对照），需要为每个 fixture 建立新的期望输出。

这部分不改变单个工具的代码量，但它让"替换"从"写代码"变成"重建验收体系"，应计入总代价。

## 五、建议顺序

1. **先做 syscap_tool**：格式已确定，一天量级，可立刻把 SDK 二进制从 6 个降到 5 个，
   并顺手验证"自研 + 官方对拍"这套方法论是否可行。
2. **restool 视收益再定**：换掉它能消除 hms `id_defined.json` 的软依赖与 restool 的平台二进制绑定，
   但要重建资源语义。若目标只是减少平台绑定，收益不足以支撑数周。
3. **es2abc 保留**：它是当前唯一真正不可替代的依赖，也是唯一值得用"官方二进制"换正确性的地方。