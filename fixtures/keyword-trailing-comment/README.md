# keyword-trailing-comment

钉住「行尾注释挂在关键字 / 右花括号上」的形态：`else // X`、`} // X`（`else` 之前）、
`} // X`（内层 if 体）、`} // X`（`build()` 体）、`return 1; // X`（普通语句）。

## 能力边界

**这条 fixture 只防「丢失」，不防「位置」。**

oracle 的注释比对按文本与条数比，不区分「行尾」与「独立行」。所以当我们的发射把
`} // BRACE_BEFORE_ELSE` 挪成 `}` 换行后另起一行 `// BRACE_BEFORE_ELSE` 时，这条 fixture
仍然是 PASS——位置错了它看不见。要查位置，得直接读 `actual/Index.ts` 与 `golden/Index.ts`
的对应行。

## 已知差异（2026-11-07）

- `ELSE_TRAILING`：已由 `process_component_build.rs` 的 `UiBranch.keyword_trailing` 承接并发出。
- `BUILD_END_TRAILING`：仍未出现，需要 `ComponentInfo` 侧提供 `build()` 体的结束位置或注释块。
- `BRACE_BEFORE_ELSE` / `INNER_BRACE`：文本在、位置不对（独立行），本 fixture 看不见。
- `STATEMENT_TRAILING`：文本在，`return 1` 的分号缺失。

