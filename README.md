# hvolt — a HarmonyOS build tool with a flat project layout

[![ci](https://github.com/hvolt-project/hvolt/actions/workflows/ci.yml/badge.svg)](https://github.com/hvolt-project/hvolt/actions/workflows/ci.yml)
[![npm](https://img.shields.io/npm/v/@hvolt/cli.svg)](https://www.npmjs.com/package/@hvolt/cli)
[![release](https://img.shields.io/github/v/release/hvolt-project/hvolt.svg)](https://github.com/hvolt-project/hvolt/releases/latest)

**H**arm**O**S **V**olt. hvolt defines its own project layout — one `hvolt.toml` per module over a
flat source tree — and generates the standard HarmonyOS project that hvigorw builds, so no
`build-profile.json5`, `module.json5`, `oh-package.json5` or `build/` tree is written by hand.
hvolt also downloads the SDK, installs hvigor and ohpm, signs (AGC or local), and pushes to a
device.

## Contents

- [Features](#features)
- [Install](#install)
- [Quick start](#quick-start)
- [Example projects](#example-projects)
- [Project layout](#project-layout)
- [Commands](#commands)
- [Requirements](#requirements)
- [Signing and AGC](#signing-and-agc)
- [DevEco Studio (optional)](#deveco-studio-optional--except-two-commands)
- [Development](#development)
- [Project status](#project-status)
- [Contributing](#contributing)
- [Acknowledgements](#acknowledgements)

## Features

- **Flat project layout** — one `hvolt.toml` per module over `src/` and `resources/`, which `hvolt assemble` compiles into the standard HarmonyOS project; no hand-written build files
- **Build chain** — translate → hvigorw → sign → push → launch, all from one CLI
- **SDK management** — list remote versions, download, import, switch, remove
- **Native (C++/NAPI)** — declare a native module as a dependency; CMake and the types bridge are wired automatically
- **Code scan** — `hvolt lint` drives DevEco's own codelinter engine over the translated tree
- **Package gate and introspection** — `hvolt check` (official app_check_tool), `hvolt hap info` (reads the package itself)
- **Signing** — OpenHarmony localSign (public test chain) in this tree; AppGallery Connect remote signing through an optional backend that is not published here
- **Devices** — hdc, cross-machine install targets, hilog, screenshot, UI tree dump, file transfer, uninstall
- **Emulator** — list / start / stop / create / delete / images / license
- **Editor support** — `hvolt lsp --stdio` serves ArkTS completion, hover, signature help,
  definition, references, outline, rename, semantic highlighting, inlay hints and call hierarchy
  out of the TypeScript build inside the installed SDK, rewrites ArkUI blocks so the service has
  scope inside `build()`, and publishes diagnostics from codelinter and from type checking,
  unsaved buffers included (see [lsp/DESIGN.md](lsp/DESIGN.md))
- **Few dependencies** — Rust to build; JDK 17+, Node.js, hvigorw and ohpm at runtime (hvolt installs the latter two)

## Editors

`hvolt lsp --stdio` is a language server; the VS Code client lives in its own repository,
[hvolt-project/hvolt-vscode](https://github.com/hvolt-project/hvolt-vscode).

```bash
hvolt lsp --stdio          # any LSP client can drive this
hvolt lsp --stdio --log-file /tmp/lsp.log   # record the protocol traffic
```

What it provides: ArkTS syntax highlighting (TypeScript grammar + ArkTS rules), completion, hover,
signature help, definition, references, outline, rename, semantic highlighting, inlay hints,
call hierarchy, folding, workspace symbols and quick fixes from the SDK's own TypeScript build
(ArkUI blocks are rewritten first, so `build()` has scope); diagnostics from codelinter and from
type checking, unsaved buffers included, mapped onto the flat sources;
`hvolt.toml` key completion, route completion, validation and dependency navigation; commands for
build, sign, install, screenshot and log. Design and measurements: [lsp/DESIGN.md](lsp/DESIGN.md).

## Install

Version numbers are `SDK version` + a release counter: **`26.0.0-0`** is the first release for SDK
26.0.0, `26.0.0-1` the next, and an SDK bump moves the whole number (`26.1.0-0`, `27.0.0-0`).
`hvolt --version` prints the same number with the SDK it defaults to:

```
$ hvolt --version
hvolt 26.0.0-0 (SDK 26.0.0)
```

A project pins its own SDK in `hvolt.toml` (`sdk_version = "26.0.0"`), so upgrading the tool never
changes what an existing project targets.

```bash
npm install -g @hvolt/cli
```

macOS arm64 and Linux x86-64 without Node.js:

```sh
curl -fsSL https://raw.githubusercontent.com/hvolt-project/hvolt/main/install.sh | sh
```

Windows PowerShell:

```powershell
irm https://raw.githubusercontent.com/hvolt-project/hvolt/main/install.ps1 | iex
```

Both scripts download the release archive for the current platform, verify it against the release's
`SHA256SUMS.txt`, and install `hvolt` into `~/.local/bin` (`%LOCALAPPDATA%\hvolt\bin` on Windows);
override with `HVOLT_PREFIX` / `-Prefix`, or pin a version with `HVOLT_VERSION` / `-Version`.

From source:

```bash
cargo install --path .      # needs Rust 1.85+
```

Archives are also published as release assets (`hvolt-<version>-<platform>.tar.gz|.zip`) for packaging.

## Quick start

```bash

# Create a project (this is the format hvolt defines)
hvolt init my-app
cd my-app

# Provide an SDK and the build tools once
hvolt sdk download 26.0.0.38      # or: hvolt sdk set /path/to/an/existing/sdk
hvolt tools install hvigor
hvolt tools install ohpm

# Translate, build, sign, push, launch
hvolt run
```

## Example projects

Both are the canonical reference for the format — each has a `README.md` next to its sources.

| Project | What it shows |
|---|---|
| [`examples/hello-world`](examples/hello-world) | The smallest project: one module, one UIAbility, one page |
| [`examples/hello-native`](examples/hello-native) | The same app plus a C++/NAPI native module (`mylib/`): `CMakeLists.txt`, `napi_init.cpp`, `Index.d.ts`, called from ArkTS |

```bash
cd examples/hello-world  && hvolt build
cd examples/hello-native && hvolt build   # C++ compiled through CMake + NDK
```

## Project layout

One application root, one directory per module:

```
my-app/
├── hvolt.toml              # application config (bundle, version, members, dependencies)
├── src/                    # a module
│   ├── hvolt.toml          # module config (entry ability, routes, device types)
│   ├── EntryAbility.ets
│   └── pages/Index.ets
├── resources/base/         # element/{string,color,float}.json + media/
└── mylib/                  # optional native module (see hello-native)
    ├── hvolt.toml          # name / type = "native" / types / version
    ├── CMakeLists.txt
    ├── napi_init.cpp
    └── Index.d.ts
```

`hvolt assemble` translates that tree into a standard HarmonyOS project under `build/`
(`build-profile.json5`, `module.json5`, `oh-package.json5`, `AppScope/`, `hvigor/`), which is what
hvigorw compiles. The flat tree is the source of truth; `build/` is generated output.

### Application config (`hvolt.toml`)

```toml
[app]
bundle = "com.example.app"
name = "my-app"
version = "1.0.0"
vendor = "example"
icon = "$media:layered_image"
sdk_version = "26.0.0"
compile_sdk_version = "26.0.0"
members = ["src"]                  # module directories

[app.signing]                      # optional
store_file = "signing/debug/debug.p12"
store_password = "..."
key_alias = "debugKey"
key_password = "..."
certpath = "signing/debug/debug.cer"
profile = "signing/debug/debug.p7b"
sign_alg = "SHA256withECDSA"

[dependencies]
# libhello = { path = "mylib", version = "1.0.0" }   # type = "native" → C++/NAPI module
```

### Module config (`src/hvolt.toml`)

```toml
[module]
name = "src"
routes = ["pages/Index.ets"]
entry = "EntryAbility.ets"
device_types = ["phone", "tablet", "2in1", "car"]
```

### Native modules

A native module is a directory with its own `hvolt.toml` (`type = "native"`), a `CMakeLists.txt`,
C++ sources and an `Index.d.ts` type declaration. It is declared **only** in `[dependencies]` —
never in `members`. The module `name` fixes the artifact: `name = "hello"` → `libhello.so`, which is
also what the CMake target, `nm_modname` and the ArkTS `import` must say. `assemble` copies the
sources to `src/main/cpp/`, generates `types/<dependency>/`, and adds
`"<dependency>.so": "file:./src/main/cpp/types/<dependency>"` to the module's `oh-package.json5`.

## Commands

### Project lifecycle

| Command | Description |
|---|---|
| `hvolt init <name>` | Scaffold a new project (this layout) |
| `hvolt assemble` | Translate the flat tree into a standard HarmonyOS project under `build/` (no hvigorw) |
| `hvolt build` | Assemble + hvigorw build + sign |
| `hvolt run [device] [-d <device>]` | Build + sign + push + launch in one shot |
| `hvolt clean` | Remove build artifacts |

### Code and package checks

| Command | Description |
|---|---|
| `hvolt lint [paths…]` | Official Code Linter over the translated tree. `--all-rules` (broad rule sets), `--json`, `--fail-on <1..3>`, `--max-defects <N>`, `--fix <paths>` (in place, explicit paths only), `--config <file>` |
| `hvolt check [--size-limit 50K]` | Package quality gate (official app_check_tool: duplicates, big files, suffix sizes) |
| `hvolt hap info [path]` | bundle / version / abilities / permissions, read from the package itself |

### SDK

| Command | Description |
|---|---|
| `hvolt sdk info` | Show SDK configuration and detected paths |
| `hvolt sdk list-remote` | List available SDK versions (per-component sizes) |
| `hvolt sdk download <version>` | Download and install an SDK version |
| `hvolt sdk import <path>` | Import an existing SDK directory into `~/.hvolt/sdk/` |
| `hvolt sdk list` | List installed versions |
| `hvolt sdk use <version>` | Switch the active version |
| `hvolt sdk remove <version> --yes` | Remove an installed version (irreversible; `--yes` required) |
| `hvolt sdk set <path> [--global]` | Use the SDK at `<path>` for this project (or for all projects) |

### Tools and configuration

| Command | Description |
|---|---|
| `hvolt tools list` | Installed tools (hvigor, ohpm, codelinter) |
| `hvolt tools list-remote` | Remotely available tool versions (npm registry + mirror) |
| `hvolt tools install <hvigor\|ohpm\|codelinter> [--version <v>] [--from <dir>] [--yes]` | Install a build tool |
| `hvolt config get` | Show all configuration values |
| `hvolt config set <key> <value>` | Keys: `sdk_path`, `node_home`, `java_home`, `ohpm_path`, `sdk_support_version`, `device_target`, `device_host`, `device_hdc` |
| `hvolt config unset <key>` | Remove a configuration value |

### Signing

| Command | Description |
|---|---|
| `hvolt sign` | Remote signing via AppGallery Connect — needs the optional backend (see [Signing and AGC](#signing-and-agc)) |
| `hvolt sign --local [--apl normal\|system_basic\|system_core] [--flavor debug\|release]` | OpenHarmony localSign with the official public test chain (dev boards / emulators / source-built ROMs; **not** retail devices) |
| `hvolt agc login\|whoami\|certs [delete <id> --yes]\|devices` | AppGallery Connect resources — needs the optional backend; cert delete is irreversible |

### Deploy and inspect on the device

| Command | Description |
|---|---|
| `hvolt push [device] [-d <device>]` | Push the newest `output/*-signed.hap` to the device |
| `hvolt apply [-d <device>]` | Quickfix (hqf) incremental patch — no reinstall; the same app identity must already be installed |
| `hvolt uninstall [bundle] [-d <device>]` | Uninstall an app (default bundle: from `hvolt.toml`) |
| `hvolt log [-T tag] [-D domain] [-k keyword] [--level D,I,W,E,F] [--bundle <b>] [--crash] [-f] [--tail <n>]` | Stream or dump hilog with filters |
| `hvolt screenshot [-o out.jpeg] [-d <device>]` | Device screenshot |
| `hvolt ui dump [-o layout.json] [-d <device>]` | ArkUI layout tree |
| `hvolt file send\|recv <a> <b> [-d <device>]` | File transfer with the device |

### Device management

| Command | Description |
|---|---|
| `hvolt device list` | List connected devices |
| `hvolt device connect <addr>` | Connect to a device over TCP |
| `hvolt device remote <host> <serial> [hdc]` | Register a device attached to another host (cross-machine install) |

### Emulator

| Command | Description |
|---|---|
| `hvolt emulator list [--details]` / `start <name>` / `stop <name>` / `delete <name>` | Manage instances |
| `hvolt emulator create <name> --os-version <v> [--type <t>]` | Create an instance (needs a downloaded image) |
| `hvolt emulator images [download <ver>]` | List system images, fetch one |
| `hvolt emulator license` | Accept the HarmonyOS emulator software agreement |

### Diagnostics

| Command | Description |
|---|---|
| `hvolt doctor` | Environment / SDK / tools / device / auth / project consistency, with a fix hint per failed check |

## Requirements

| Dependency | Version | Notes |
|---|---|---|
| Rust | 1.85+ | Build-time only (clap 4.6 MSRV) |
| JDK | 17+ | Required by hvigorw |
| Node.js | 20+ | Required by hvigorw and ohpm |
| hvigorw | — | `hvolt tools install hvigor` |
| ohpm | — | `hvolt tools install ohpm` |
| HarmonyOS SDK | 26.x recommended | `hvolt sdk download <version>`; the remote list covers API 10–26, and anything below 26 is flagged as deprecated |

## Signing and AGC

**Local signing (in this tree).** `hvolt sign --local` uses OpenHarmony's public test certificate
chain, fetched from the OpenHarmony repository (Apache-2.0) and pinned by SHA-256. That identity is
pre-trusted only by development boards, emulators and source-built ROMs; a retail HarmonyOS device
needs a certificate and provisioning profile issued to a Huawei developer account.

**Remote signing through AppGallery Connect (not in this tree).** `hvolt sign` without `--local` and
the `hvolt agc` commands need a backend that talks to AppGallery Connect — the login flow, token
handling and the certificate / device / provisioning requests that DevEco Studio performs. That
backend is **not published here**: this repository contains no AGC endpoints, no login flow and no
request/response handling for those services. The commands stay in the CLI surface and report that
the backend is absent, so scripts fail loudly instead of silently doing nothing.

If you maintain your own AppGallery Connect integration, it can be compiled in as an overlay: place
it in `agc-local/` next to the crate and build with `cargo build --features agc-local`. The overlay
is a private artefact; it is not part of this repository and not accepted as a contribution. Overlay
builds additionally mask their string constants at compile time and ship stripped symbols, so the
protocol details do not show up in `strings` output — that raises the cost of inspection, it is not
a security boundary, and no one should rely on it as one.

Whatever path you use, the terms of the Huawei developer services you authenticate against apply, and
signing material you generate stays under `~/.hvolt/` and in your project directory.

## DevEco Studio (optional — except two commands)

hvolt downloads its own SDK and build tools, so **no DevEco Studio install is needed** for
`init`, `assemble`, `build`, `sign`, `push`, `run`, `check`, `hap`, `log`, `screenshot`, `ui`,
`file`, `uninstall`, `device`, `sdk`, `tools`, `config`, `doctor` or `agc`.
Two features are the exception:

| Command | What it needs from DevEco Studio | After that |
|---|---|---|
| `hvolt lint` | the `plugins/codelinter` payload (~200 MB). Huawei publishes it only inside DevEco Studio — npm has no `@ohos/codelinter` (404) and the SDK component list carries only ets/js/native/previewer/toolchains (both measured) | copy it once from a machine that has DevEco: `hvolt tools install codelinter --from <DevEco>/plugins/codelinter` — `hvolt lint` then runs on a DevEco-free machine (measured on Windows and Linux) |
| `hvolt emulator ...` | the emulator binary and system images, which ship only with DevEco Studio on Windows/macOS | install DevEco there, or use a physical device — Linux has no official emulator binary at all |

Where DevEco *is* installed, hvolt also uses its SDK or `hdc` as a fallback, but never requires
them: `hvolt sdk download` and the SDK's own `toolchains/hdc` are the primary path.

## Development

```bash
git clone https://github.com/hvolt-project/hvolt.git
cd hvolt
cargo build
cargo test
```

Three checks gate every change; all must pass before a pull request:

```bash
cargo fmt --all --check                      # empty output
cargo clippy --all-targets -- -D warnings    # "Finished"
cargo test                                   # all passing
```

Conventions this codebase relies on: no compatibility shells (a binary crate has no downstream
API), no swallowed `Result` (`let _ = …` needs a stated reason), a comment that names a CLI command
must match `hvolt --help`, and a comment claiming a mechanism carries the measurement that produced
it. Details and examples in [CONTRIBUTING.md](CONTRIBUTING.md).

## Project status

`0.1.0` — usable end to end (translate, build, sign, deploy, lint, device tooling), but pre-1.0:
the CLI surface may still change between minor versions. hvolt is an independent community project,
not affiliated with or endorsed by Huawei; "HarmonyOS", "DevEco Studio" and "Huawei" are trademarks
of Huawei Technologies and are used here only to describe compatibility.

## Contributing

Bug reports and pull requests are welcome. [CONTRIBUTING.md](CONTRIBUTING.md) lists the checks a
change must pass; report vulnerabilities privately as described in [SECURITY.md](SECURITY.md);
release notes live in [CHANGELOG.md](CHANGELOG.md). By participating you agree to the
[Code of Conduct](CODE_OF_CONDUCT.md).

## Acknowledgements

Built in pair-programming sessions with the **deepseek-flash** model (DeepSeek). Those sessions set
the working habit this codebase keeps: a claim about how something behaves comes with the
measurement behind it, and a wrong assumption gets settled by running the code, not by arguing
about it.

## License

GPL-3.0-or-later — see [LICENSE](LICENSE). Third-party components and the origin of fetched
artifacts are listed in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
