# hvolt clean-code 规范

适用于 `src/**`。改代码前必读；提交前必须让三道闸全绿。

## 1. 验收命令（三条全绿才提交）

```bash
cargo fmt --all --check                      # 空输出 = 通过
cargo clippy --all-targets -- -D warnings    # 末行 "Finished" = 通过
cargo test                                   # 全部通过 = 通过
```

口径由 `rustfmt.toml` 与 `Cargo.toml` 的 `[lints]` 固定，不靠人工记忆。
任一条不过，不提交。

## 2. 格式化

- 只认 `rustfmt.toml`（stable 选项，`edition=2021`、`max_width=100`、`use_field_init_shorthand`）。
- 不手写列对齐、不用 tab、不留 `cargo fmt` 可改的 diff。

## 3. 命名与模块

- 一个模块一件事：`src/lint.rs`=扫描，`src/sdk.rs`=SDK 解析/下载，`src/tools.rs`=工具安装；新职责新文件，不塞进 `main.rs`。
- 文件首行用一行 `// <域>: <干什么>` 起头（如 `// AGC: manage AppGallery Connect resources`）。
- `pub` 只给 `main.rs`/`cli.rs` 分派需要的入口；其余用 `pub(crate)` 或私有。二进制 crate 没有下游消费者，`pub` 不是 API 承诺。
- CLI 子命令动词与 `src/cli.rs` 的变体一一对应，不另造同义词。

## 4. 错误处理

- 到 CLI 边界统一 `Result<(), String>`；错误串必须带上下文：`format!("read {}: {}", path.display(), e)`。
- **禁止 `let _ = <Result>` 吞错**。可忽略的失败要么 `eprintln!("Warning: ...")`，要么紧邻一行注释说明为什么能丢（例：`let _ = fs::remove_file(...)` 清理临时文件）。
- `expect()` 只用于「失败即不可能」的位置，消息写不变量（`"create element dir"`），不写 `"failed"`。
- 用户输入错误走 `Err(...)`，不 `panic!`。

## 5. 规模与类型

- 参数 > 7 个：优先抽结构体；确属 CLI 透传（一个参数对一个 flag）时用
  `#[allow(clippy::too_many_arguments)]` + 同一行理由。
- 返回元组 ≥ 3 元素：起 `type` 别名（现有：`ToolPaths`、`ModuleSets`）。
- 单函数 > 约 80 行：拆分，或在函数头注明为什么不能拆。

## 6. 死代码

- 不留 "legacy entry point / kept for X wiring" 之类的兼容壳——需要时从 git 历史取回。
- 必须保留的：平台门控函数、尚未接线的保留方言，用 `#[allow(dead_code)] // 理由`。

## 7. 文档注释

- 公开项一行摘要 + 必要时机制段；不写空话。
- 注释里出现的 CLI 命令必须与 `hvolt --help` 实际一致：删/改命令的同一次提交里同步注释。
- 机制段写「测得的机制 + 日期」，不写猜测；不确定就写「未验证」。
- 用技术陈述：不写第一人称（we/our/my）、不写内部工单代号（`audit-1`、`A/C-1` 之类）、不写隐喻或俚语
  （honest / disease / explode / litter）。需要承载历史时用「(measured)」+ 日期。
- 机器核对：

  ```bash
  grep -rnE '^\s*(///|//|//!).*(audit[- ]?[0-9#]|\b(we|our|my|I)\b|honest|disease|explode|litter)' src/
  ```

## 8. unsafe 与平台分支

- 全 crate **只有一处** `unsafe`：`main.rs` 里把 SIGPIPE 复位为 `SIG_DFL`（`hvolt ... | head` 不 panic）。
  它就地带 `#[allow(unsafe_code)]` + 理由；新增 `unsafe` 必须在本文登记。
- `#[cfg(windows)]` / `#[cfg(unix)]` 两支都要能独立读懂：目录用 `remove_dir`、符号链接用 `remove_file` 这类差异，注释写明为什么。

## 9. 测试

- 与实现同文件 `mod tests`，且放在文件**末尾**（`clippy::items_after_test_module` 会拦）。
- 新增行为必须带测试；测试名描述行为（`rejects_dot_dot_entry`），不描述实现细节。

## 10. 当前 allow 清单（审计用）

| 位置 | 属性 | 理由 |
|---|---|---|
| `src/lint.rs` `build_args` | `clippy::too_many_arguments` | 一个参数对 codelinter 一个 flag，原样透传 |
| `src/sign.rs` ×4（`create_provision`/`finalize_signing`/`sign_app`/`sign_package`） | `clippy::too_many_arguments` | 与签名 CLI 表面 1:1 |
| `src/main.rs` `reset_sigpipe` | `rustc::unsafe_code` | 唯一的 SIGPIPE 复位，见 §8 |
| `src/target.rs` `Shell::PowerShell` | `dead_code` | 保留方言：pwsh 默认主机接入后即用 |

新增条目必须同时改这张表。

## 11. 提交前清单

1. `cargo fmt --all --check` → 空输出
2. `cargo clippy --all-targets -- -D warnings` → Finished
3. `cargo test` → 全部通过（改动后测试数不得少于改动前）
4. 动过 CLI 或注释里的命令 → 用 `hvolt --help` 复核
5. 动过注释或文档 → 跑 §7 的 grep，输出只允许剩下引号内的示例文本
6. 动过行为 → 同轮跑一次真实调用并把读数写进 PR 描述
7. 动过结构（合并重复、抽接缝、改签名）→ 跑 §12 的三项判据并给出读数

## 12. 结构判据（可机械核对）

用于判断一次改动是否让结构变好，而不是只让测试变多。任一项不达标即为未完成的重构。

### 12.1 单一真源计数

同一个策略——默认值、参数表、判定规则、路径拼装——在 crate 内只允许一处实现。
第二份出现即缺陷：删掉，或在注释里写明两份为何必须并存（例如平台分支）。

```bash
# 默认值与 runtimeOS 字面量，带上上下文一行，便于确认是否附了指回真源的注释
grep -rn -B1 '"HarmonyOS"\|"OpenHarmony"' src --include='*.rs' \
  | grep -v test | grep -v '^src/config.rs' | grep -v '^--$'
```

判据：命中为 0，或每条命中上方紧邻一行注释说明它为何不是 runtimeOS 策略。
当前两处合规例外：`src/sign_local.rs` 的 profile `developer-id`、`src/translator/strategy.rs`
的 signingConfigs `type`，均为签名配置的固定字段。

真源位置：runtimeOS 的默认值与比较在 `src/config.rs`（`DEFAULT_RUNTIME_OS`、
`effective_runtime_os`、`is_open_harmony`）；平台判断在 `src/platform.rs`。

### 12.2 接缝纯度

判定与 IO 分离：一个函数要么决定「该不该做、是不是错」，要么执行「读写、起进程」，
不在同一函数里既判定又执行。

- 判定函数：签名与函数体内不出现 `&Path`、`Command`、`std::fs`、`SdkEnv`。
- IO 函数：不自行推导策略；策略来自入参或专门的纯函数。

```bash
# 纯函数的签名只含数据
grep -rn 'pub(crate) fn ' src/*.rs | grep -vE '&Path|Command|SdkEnv'
```

判据：命中项必须满足其一——
(a) 函数体只做数据准备（候选路径列表、参数打包），无业务判定；或
(b) 业务判定被抽到同文件的无 IO 函数里，并带单元测试。

路径候选列表、curl 参数打包中的 `if`/`match` 属 (a)，不算规则。
反例：`assemble_and_build` 曾把四处 `process::exit` 与构建设置混在一起，判定无法单独调用，
属 (b) 不成立。

### 12.3 改动可回退粒度

一个提交只承载一类改动：收敛重复、抽出接缝、修复缺陷三者不混提。
混提后无法单独回退其中一类，评审也无法逐条核对。

判据：`git show --stat <commit>` 的文件集合若同时包含「仅搬移/重命名」与「行为变更」两类，
即判为不达标，拆分后重提。

已按本条拆分的实例：`sign.rs` 的 `SignRequest` 收敛、`build_hvigor::strip_signing_configs`
的字符串感知修复、`platform` 判断收敛，属三类改动，应分三次提交。

### 12.4 行为变更须显式报备

结构改动若顺带改变行为（例如修复了原实现会写坏文件的扫描逻辑），必须在 PR 描述里单独列一段：
症状、根因、修法、复验读数。不得并入「重构」一并交付。

```bash
# 改动前后各跑一次真实调用，保存原始输出
```

判据：PR 描述中存在「行为变更」段落，且带命令原始输出。

