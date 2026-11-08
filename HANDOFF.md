# Handoff — hvolt 26.0.0-9

工作区：`/Users/hongyuncong/workspace/better-harmony-app-build`
当前基线：`main` = `9bb5c80`（ci: build the darwin lane on macos-15 (#45)）

## 状态

- **26.0.0-9 已发布**：GitHub release `v26.0.0-9`（三个构件 + `SHA256SUMS.txt`），npm `@hvolt/cli` 与三个平台包均为 `26.0.0-9`。
- PR 43（#43）已合并：资源编译树内化、SDK 平台二进制收敛到 `es2abc` + `llvm-strip`、npm 平台包对齐、产品变体、`src/` 拆领域模块、命令改名 `build`/`migrate`。
- PR 44（#44）已合并：版本号 `26.0.0-9`，覆盖 `Cargo.toml`、`Cargo.lock`、`npm/package.json` 与五个 `npm/platforms/*`。
- PR 45（#45）已合并：darwin 构建从 `macos-14` 迁到 `macos-15`（前者被 GitHub 退役）。
- 发布由 `release.yml` 驱动：`main` 上版本比最新 tag 新就自动构建 + GitHub release + npm publish，不靠手动打 tag。

本文件替代此前那份「自建 ArkTS 编译链」handoff。旧的逆向细节已落进代码与 `docs/agents/`，最新状态以仓库为准。

## 发布链上最重要的一个坑：私有 overlay

`agc-local/` **不在仓库里**（`.gitignore:33`），它是 AGC 后端，CI 从 secret `AGC_OVERLAY_B64` 解出 tar.gz 到仓库根，再以 `--features agc-local` 编译。

后果：**仓库代码里 `src/sign/backend.rs` 的调用面与 secret 里的 overlay 必须同时更新**。只改一边，本地因为 gitignore 里那份旧副本存在而照样能编过，CI 上三个目标机会一起炸。

本次就是这样失败的：`backend.rs` 调用 `add_device_cli` / `delete_device_cli`，secret（2026-10-24 版）提供的还是底层 `add_device`，linux-x64 与 win32-x64 报 `cannot find function`，darwin-arm64 被连带取消，publish 跳过。

判据：改 `src/sign/**` 或 `agc-local/**` 任一方的接口后，必须同步更新 secret：

```
tar -czf /tmp/agc-overlay.tgz agc-local
base64 -i /tmp/agc-overlay.tgz -o /tmp/agc-overlay.b64
gh secret set AGC_OVERLAY_B64 --body "$(cat /tmp/agc-overlay.b64)"
```

本地验证覆盖不到这条：`cargo check --features agc-local` 用的是磁盘上那份，不是 secret。

## 推 GitHub 的通道

`github.com:443` 的 https 在本机不通（HTTP2 framing）。传输走 git over SSH-443：

```
GIT_SSH_COMMAND='ssh -o StrictHostKeyChecking=no -p 443' \
  git push ssh://git@ssh.github.com/hvolt-project/hvolt.git <branch>
```

`gh` 只管 API（`pr create/view/checks/merge`、`run list/watch`、`secret set`）。gh 不能传输 git 对象：远端没有该对象时 `git/commits/<sha>` 返回 404，`POST /git/refs` 只能指向已存在的对象。

注意 `git fetch` 同样要带 `GIT_SSH_COMMAND`，否则默认 22 端口连不上，报的是「检查访问权限」而不是网络错误。

## 本次的行为教训

1. **本职环节不转审批项。** 传输、构建、验 CI 是代理职守；把它们拆成「请批准」「你跑这个脚本」等于把活推回去。
2. **失败两次换判据。** 同一命令连撞三次以上，说明没在判断错误类型。
3. **交付前对账。** PR 正文要覆盖 `git log` 的全部提交，模板三段要照抄仓库 `.github/PULL_REQUEST_TEMPLATE.md`。
4. **CI 与本地不一致时用环境差异复现。** 本次两个测试分别依赖私有绝对路径与本机 SDK，修法不是跳过而是换仓库内 fixture + 在无 SDK 的 HOME 下实测。
5. **`git reset --hard` 前先看 `git status`。** 一次误操作丢掉了工作区里未提交的 `AGENTS.md` 与 `HANDOFF.md` 改动（前者无法恢复）。

## 工作区未提交项

`docs/agents/resources-index.md`、`docs/agents/sdk-dependencies.md` 未跟踪（agent 侧文档，按既定标准不进 PR）。`.scratch/` 下是临时脚本。

## 建议技能

- `harmony-build` — hvolt 构建、装包、抓日志
- `arkts-pitfalls` — 改 ArkTS 编译相关代码
- `docs/agents/clean-code.md` — 提交闸（fmt / clippy `-D warnings` / test 三绿）

## 下次接手先做

1. `gh run list --workflow=release.yml --limit 3` 确认 26.0.0-9 的发布结果
2. `gh release view v26.0.0-9`、`npm view @hvolt/cli version` 核对两个出口都到位
3. 改任何 `sign/**` 或 `agc-local/**` 之前，先读上面「私有 overlay」一节
