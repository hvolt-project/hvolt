# Changelog

All notable changes to hvolt are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow
[Semantic Versioning](https://semver.org/spec/v2.0.0.html). While the project is pre-1.0, minor
versions may change the CLI surface.

## [Unreleased]

## [0.1.0] — 2026-10-18

First public release: a HarmonyOS build tool with its own flat project layout.

- Prebuilt binaries for **darwin-arm64**, **linux-x64**, **win32-x64**, published to npm as
  `@hvolt/cli` and attached to the GitHub release.
- Targets SDK **26.0.0** (hvolt 0.1.x → SDK 26.0.0 pairing is fixed).

### Added

- **Flat project layout** — one `hvolt.toml` per module, a flat tree (`hvolt.toml` per module, `src/`, `resources/`) translated into a
  standard HarmonyOS project; `hvolt init` scaffolds it, `hvolt assemble` generates `build/`.
- **Build chain** — `assemble` → hvigorw → sign → push → launch (`hvolt build`, `hvolt run`), with
  signing configuration managed in `hvolt.toml`.
- **Native modules** — C++/NAPI modules declared as dependencies; CMake wiring and the
  `Index.d.ts` types bridge are generated. Demonstrated in `examples/hello-native`.
- **SDK management** — `sdk list-remote`, `download` (per-component sizes, resumable), `import`,
  `list`, `use`, `remove`, `set`.
- **Tool management** — `tools list`, `list-remote`, `install` for `hvigor`, `ohpm` and the
  Code Linter payload (`--from <dir>`).
- **Code scan** — `hvolt lint` drives DevEco's codelinter engine over the translated tree, with
  `--all-rules`, `--json`, `--fail-on`, `--max-defects` and `--fix`. Engines that fail to run are
  reported as an error instead of a clean pass.
- **Package checks** — `hvolt check` (official app_check_tool) and `hvolt hap info` (bundle,
  version, abilities and permissions read from the package).
- **Signing** — `sign --local` (OpenHarmony localSign with the public test chain) for development
  boards, emulators and source-built ROMs. Remote signing through AppGallery Connect is provided by
  an optional backend that is not part of this source tree.
- **Device tooling** — `push`, `apply` (hqf quickfix), `uninstall`, `log` (hilog filters),
  `screenshot`, `ui dump`, `file send/recv`, `device list/connect/remote`.
- **Emulator** — `list`, `start`, `stop`, `create`, `delete`, `images download`, `license`.
- **Diagnostics** — `hvolt doctor` checks environment, SDK, tools, device, auth and project
  consistency, with a fix hint per failed check.
- **Examples** — `examples/hello-world` and `examples/hello-native`.

### Notes

- Requests to Huawei endpoints identify hvolt in `User-Agent` (no client impersonation), and the
  icon assets shipped with the scaffold are original works.
- `hvolt lint` needs the codelinter payload, which Huawei ships only inside DevEco Studio; the
  emulator needs DevEco's emulator binary and images. Every other command runs without DevEco
  Studio. See the README section "DevEco Studio (optional)".
