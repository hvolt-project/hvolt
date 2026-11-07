## Agent skills

### Issue tracker

Issues live as local markdown files under `.scratch/<feature-slug>/`. See `docs/agents/issue-tracker.md`.

### Triage labels

All five triage roles use their canonical names (`needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`). See `docs/agents/triage-labels.md`.

### HarmonyOS device flow

真机装包跨三台机器（构建/签名宿主、持有设备的主机、本机）与两条签名链（localSign 只被开发板信任，零售机必须走 AGC 覆盖层）。改构建/签名/推包相关的东西、或要往设备上装包之前，先读拓扑与命令：See `docs/agents/harmony-device-flow.md`.

### Domain docs

Single-context layout — one `CONTEXT.md` + `docs/adr/` at the repo root. See `docs/agents/domain.md`.

### hvolt codebase

hvolt 的架构、CLI 命令参考、配置系统和开发指南。在修改 hvolt 代码或添加新功能前参阅。See `docs/agents/hvolt-skill.md`.

### Clean code

代码规范与验收闸（格式化、clippy 严格模式、测试三道全绿才提交）。改 `src/**` 前参阅。See `docs/agents/clean-code.md`.
