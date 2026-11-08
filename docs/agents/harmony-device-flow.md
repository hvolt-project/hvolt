# 鸿蒙真机装包：谁构建、谁签名、谁推包

这份文档记录跨主机的工作拓扑。它存在的原因是：拓扑只在会话里说过一次，压缩之后就丢了，之后的 agent 会把「更新远端 hvolt」理解成「在本机造一个 Windows 二进制」，然后去装 mingw、改 CI、甚至提议用 DevEco 图形界面——四条路都绕开了既有工具链。

## 三台机器的职责

| 主机 | 地址 | 有什么 | 负责 |
|---|---|---|---|
| GPU 服务器 | `hongyuncong@192.168.3.4:52525` | Rust、SDK（`~/.hvolt/sdk` 26.0.0.38）、AGC 登录态 | **构建、签名** |
| macOS（本机） | 本机 | hvolt、SDK 编译器（es2abc/restool）、`~/DevEcoStudioProjects/` | 转发文件；同网段时也能直连设备 |
| Windows Surface | `xyzsu@100.64.0.2`（tailnet） | `~/.local/bin/hvolt.exe`、SDK 编译器、USB/无线连真机 | 推包 |
| 真机 | `192.168.1.3:38565`（PLR-AL00，API 26） | — | 安装与运行 |

**关键约束**：GPU 服务器到不了 Surface（不在同一 tailnet）。

## 签名有两条链，选错就装不上

| 链 | 命令 | 被谁信任 |
|---|---|---|
| OpenHarmony localSign | `hvolt sign --local` | 开发板 / 模拟器 / 自编译 ROM。**零售机拒**：`code:9568257 error: fail to verify pkcs7 file` |
| AppGallery Connect | `hvolt sign`（无 `--local`） | 华为开发者账号签发的证书与 profile，零售机可装 |

AGC 后端是 `agc-local/` 覆盖层，**不在公开源码树里**（`Cargo.toml` 的 `agc-local` feature）。没有它时 `hvolt agc` 与 `hvolt sign` 会明确报「后端缺失」。

```bash
cargo build --release --features agc-local        # 需要 agc-local/ 目录在场
```

## 完整流程（2026-10-24 实跑）

```bash
# 1 构建（服务器）
cd ~/work/hvolt-current && ./target/release/hvolt build --cache-dir /tmp/hv-cache

# 2 签名（服务器；设备在别处，用覆盖层预留的跨机入口交 UDID）
HVOLT_DEVICE_UDID=$UDID ./target/release/hvolt sign

# 3 传递（服务器 → 本机 → Surface，服务器到不了 Surface）
scp output/*-signed.hap <mac>:/tmp/ && scp /tmp/*-signed.hap xyzsu@100.64.0.2:C:/tmp/

# 4 安装与取证（Surface）
hvolt push C:/tmp/<name>-signed.hap -d <serial|ip:port>
hvolt log -d <serial|ip:port> -T <tag>
```

## 签名分两步，别在 `sign` 之后就找包

`hvolt sign` 只做三件事：生成 keystore 与 CSR、向 AGC 换证书与 provisioning profile、把
`[app.signing.default]` 写进 `hvolt.toml`。**它不签 HAP。** HAP 的签名发生在下一次
`hvolt build`（输出里出现 `Signing HAP...  ✓ <bundle>-signed.hap`）。

```bash
hvolt build                 # 出 unsigned HAP
hvolt sign                  # 换证书 + profile + 写配置（设备在别处时配合 HVOLT_DEVICE_UDID）
hvolt build                 # ← 这一步才签出 signed HAP
```

## 已遇到的问题（都经过实测）

- **不要替换 Surface 上的 `hvolt.exe` 而不知道它是什么构建**。2026-10-24 我用 CI 的普通构建覆盖了它（自称 `0.1.0` 说明那是另一条构建线，普通构建打印 `26.0.0-1 (SDK 26.0.0)`），备份留在 `~/.local/bin/hvolt.exe.bak-20261024-1050`。
- **`runtime_os` 与 `device_types` 必须匹配 SDK**：没有 hms 的机器上 `runtime_os = "HarmonyOS"` 会报 `00303168 SDK component missing`；改成 `OpenHarmony` 后 `device_types` 只能用 `[default, tablet, tv, wearable, car, 2in1]`，`phone` 会被拒。`hvolt doctor` 会把这条直接写出来，先跑它。
- **AGC 登录态会过期**：`hvolt agc login` 打印一个 `apply?port=<port>` 的链接，需要在本机浏览器打开完成回调；refresh token 过期时报 `jwToken check failed … 401`，只能重新登录。
- **`~/.hvolt` 可能不可写**（沙箱或权限）：把 `HOME` 指到工作区内一份副本跑，登录态与签名材料都会落在那里。
- **生成的 keystore 口令不能是纯十六进制**：`build` 把「32 位以上全十六进制」判成 DevEco 编码口令并拒绝签名，而早期 AGC 生成器产出的正是 64 位十六进制——工具拒绝自己刚生成的密码。覆盖层生成器已改为非十六进制字母表（26.0.0-3 起）。手工绕过：`keytool -importkeystore` 重包成非十六进制口令，再同步改 `hvolt.toml`。
- **发布版是否带 AGC 要当场验**：`hvolt agc whoami` 返回账号＝后端在；返回 `this build has no AppGallery Connect backend` ＝公开构建。覆盖层通过仓库 secret `AGC_OVERLAY_B64` 在 CI 侧解码，源码不在仓库里。
- **无线调试会掉线**：`hvolt device list` 看不到目标时，先 `hvolt device connect <ip:port>` 校验可达并记入配置。
- **`hvolt sign` 在设备不可见时报 `No device UDID found`**：把真机 UDID 通过 `HVOLT_DEVICE_UDID` 交给签名端（UDID 由持有设备的主机取得）。
