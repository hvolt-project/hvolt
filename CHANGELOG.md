# Changelog

All notable changes to hvolt are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow
[Semantic Versioning](https://semver.org/spec/v2.0.0.html). While the project is pre-1.0, minor
versions may change the CLI surface.

## [Unreleased]

### Added

- A VS Code extension in `editor/vscode`: registers `.ets`, launches `hvolt lsp --stdio`, shows
  build state in the status bar and exposes Build / Run / Sign / Install / Screenshot / Log.

- `hvolt lsp --stdio`: a language server for editors.
- Palette commands in the language server: `hvolt.build`, `assemble`, `sign`, `push`, `run`,
  `screenshot`, `log` — each returns `{ok, seconds, tail}` (success taken from the exit code) and
  posts a `window/showMessage` notification.
- Diagnostics in the language server come from codelinter (`hvolt lint --json`, on open/save) and from
  a real build (`hvolt.buildDiagnostics` command); both are mapped from the generated trees back to
  the flat project paths and merged per source. Semantics (hover, completion, definition,
  signature help, rename) come from the TypeScript build inside the installed SDK, which parses
  `.ets` and ArkUI structs; diagnostics deliberately stay with hvigor, codelinter and
  `app_check_tool`, because the fork alone mis-reports legal ArkUI block-chains
  (`Column() { … }.width()`). See `lsp/DESIGN.md`.

## [26.0.0-0] — 2026-10-18

Version numbering switches to "SDK version + release counter" (`26.0.0-0`; see RELEASING.md).
`hvolt --version` now prints the SDK the build defaults to. Behaviour is otherwise the same as 0.1.0,
plus the fixes below.

### Fixed

- `hvolt tools install ohpm` no longer hard-fails when the mirror drops the pinned 5.0.2 archive: it
  falls back to the newest small, host-matching build and says which one it picked.
- releases upload their `SHA256SUMS.txt`, and the installers verify downloads against it.
- an SDK skeleton left behind by an uninstalled DevEco Studio no longer shadows `~/.hvolt/sdk`.

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
