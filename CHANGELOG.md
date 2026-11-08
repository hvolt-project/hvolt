# Changelog

All notable changes to hvolt are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow
[Semantic Versioning](https://semver.org/spec/v2.0.0.html). While the project is pre-1.0, minor
versions may change the CLI surface.

## [Unreleased]

### Changed

- A prebuilt binary is published and installable for `linux-arm64`, built on a native arm64
  runner. `install.sh` resolves `aarch64`/`arm64` to it and names the real support list when it
  refuses a platform.
- `@hvolt/cli-darwin-x64` is no longer declared. The launcher listed five platform packages
  while the release built three, so an Intel Mac installed a package with no binary behind it.

## [26.0.0-9] — 2026-11-08

### Added

- `hvolt dep sync [DIR]` turns registry packages that are already on disk into
  `deps/<scope>/<name>/`: sources move to the package root, specifiers that addressed
  `src/main/ets/` are rewritten, each package gets its own `hvolt.toml` (`type` = `arkts` /
  `bytecode` / `js`), and the root `[dependencies]` switches to `path`. No network, no registry
  lookup.
- Hidden `hvolt abc-records <FILE>` lists the records of a `modules.abc` with the source file
  each one came from, without the SDK's `ark_disasm`.
- `hvolt dep sync` is the only writer of `deps/`; the build resolves a package's kind, entry and
  version from its own `hvolt.toml` or its directory shape.

### Changed

- Device access speaks the hdc wire protocol in process: the handshake (RSA-PSS over SHA512
  with a fresh session id), `shell`, file send and install all run against the device's `hdcd`
  without spawning `hdc`.
- The build reads no hvigor manifest: `oh-package.json5`, `build-profile.json5`, `package.json`
  and `hvigorfile.ts` are not configuration sources, and a bare import the `[dependencies]` table
  does not cover is an error naming the package instead of being resolved from a leftover
  manifest. `src/main/ets` no longer exists inside dependencies.
- Byte-code dependencies carry `modules.abc` at the package root (`deps/<x>/modules.abc`), not
  under `ets/`.
- API level → bytecode version comes from an in-tree table (`src/build/deps/sdk_bytecode.rs`, 19
  rows) instead of running the SDK's `ts2abc.js` through Node.
- `rpcid.sc` is encoded in-tree (`src/build/artifacts/rpcid_sc.rs`) instead of invoking the SDK's
  `syscap_tool`. Byte-identical to the tool's output on the measured fixture (52742 bytes,
  sha256 `56435e3f…690e8f`).
- Signing, packing and package checking no longer run the SDK's Java tools. `hvolt sign` uses
  `hapsigner` in process for localSign, for signing the provisioning profile, and for the
  acceptance checks; `hvolt check` computes duplicate / oversized / per-suffix statistics from
  the archive directory instead of parsing the tool's `stat.json`; the hidden `hvolt pack-hap`
  assembles a HAP from hvigor intermediates.

### Fixed

- `hvolt doctor` no longer reports a device-type error for library modules: `type = "har"` /
  `"hsp"` members ship no device profile, so their `device_types` list is not a claim the
  installed SDK has to satisfy.
- `hvolt check --size-limit` compares bytes. The SDK tool's `--stat-file-size` is in KB, so the
  previous pass-through made every threshold 1024x larger than requested.
- The provisioning profile carries the leaf certificate only. The whole chain used to be written
  into `bundle-info.distribution-certificate`, which the SDK tool silently truncated.
- Key aliases resolve case-insensitively against the p12 store: the official test store holds
  them lower-case while callers name them as the SDK documents them.

## [26.0.0-8] — 2026-11-01

### Added

- `hvolt sign <HAP>` signs a bare unsigned package where it lies, with no project and no
  build: the result stays in the package's own directory as `<name>-signed.hap` (an input
  ending in `-unsigned` becomes `-signed`). Without `--p12/--profile/--cert` the package is
  signed with the localSign test chain, bound to the `bundleName` it declares (`--bundle`
  covers a package that declares none); giving all three materials signs with those instead.
  The result is verified with `hap-sign-tool verify-app` before the command reports success.
- `hvolt push --hap <FILE>` installs a package at an arbitrary path. An unsigned package is
  refused before reaching the device transport, which would otherwise only report a signature error.

### Fixed

- Signedness is now read from the archive (`META-INF/*.sig|.rsa|.ec`) instead of the file
  name, so a package named `*-signed.hap` whose contents carry no signature is still signed,
  and one that really is signed is refused rather than overwritten.
- Module-level `metadata` is no longer dropped from the generated `module.json5`. `resource`
  and `value` are emitted as the mutually exclusive fields they are, and the list no longer
  assumes a preceding section — a non-entry module used to produce a closing bracket followed
  by a comma, which `hvigor` rejects as invalid JSON.
- A dependency declared with parent components now resolves to its mirrored copy under
  `build/<basename>/` instead of overshooting one level above `build/` into the source tree;
  `.har` file dependencies keep their declared path verbatim, and a dependency with no local
  path is emitted as its version rather than a `file:` path to a directory that does not exist.
- `libs/` content that `hvigor` will drop (anything but `*.so`) is announced at assemble time
  with the source-to-destination mapping, instead of being silently left out of the package.
- Multi-device ambiguity errors list the connected devices and both remedies (`hvolt device
  connect <serial>`, or `-d <serial>` per command).

## [26.0.0-7] — 2026-10-27

### Added

- `hvolt skill` — the binary embeds its own documentation (usage/architecture guide, code
  conventions, cross-host device flow) so any machine with hvolt has the docs without a clone
  or network. `hvolt skill list` (with `--plain` / `--json`), `hvolt skill show <id>`, a content
  hash so the same version with different docs is distinguishable, and SIGPIPE-safe output for
  agent consumers. The embedded docs are a community-sanitised set: no private hosts, IPs, user
  names or device identifiers (enforced by a leak-scan test).

### Fixed

- Incremental rebuild (`hvolt assemble` mirror path) now propagates native source changes into
  the build tree. Previously the mirror skipped files that already existed: edited `.cpp`/`.h`
  never reached `src/main/cpp/<module>/`, `Index.d.ts` edits never reached the types bridge,
  and deleted sources stayed in the build tree — so "rebuild" silently produced yesterday's
  package until `build/` was wiped. The mirror and full-translate paths now share the native
  copy, types-bridge and stale-prune helpers; the top-level CMakeLists aggregator also rewrites
  itself when the native module list changes (module-owned inline_cpp trees stay untouched).

## [26.0.0-6] — 2026-10-26

### Fixed

- Building a project with several native modules no longer drops all but the last one: their C++
  sources were flat-copied into a single `src/main/cpp/`, so same-named files (`napi_init.cpp`,
  `CMakeLists.txt`) overwrote each other and only the last module compiled. Each native module now
  mirrors into its own `src/main/cpp/<module>/` subdirectory, wired together by a generated
  top-level `CMakeLists.txt` (`add_subdirectory` in declaration order; an existing module-owned
  file is left untouched).
- Headers (`.h`/`.hpp`) are now carried into the build tree with the C++ sources; they were
  skipped entirely, so any native module split across `.h`/`.cpp` failed to compile.
- `hvolt build` no longer fails with hvigor 00303208 "Unable to find 'sdk.dir'" when the SDK is
  only set via `DEVECO_SDK_HOME`: `run_assemble` writes `local.properties` (`sdk.dir=`), as DevEco
  Studio does.
- The hvigor wrapper no longer fails resolving `@ohos/hvigor-ohos-plugin` (`ERR_PNPM_FETCH_404`
  — the plugin is not on the public registry): the declared dependency is rebound to the vendored
  copy shipped with hvolt (a `file:` dependency, forward-slash paths on Windows too).
- Shared/HSP/HAR `oh-package.json5` `main` now resolves from `src/main/`, where the entry file is
  actually mirrored; the old bare `Index.ets` pointed at a file that never existed.

### Changed

- `module.json5` for entry modules now emits `isolationMode: isolationFirst`.
- The SDK platform version and the hvigor plugin version each have a single source of truth in the
  code, with a drift-guard test (`hvolt --version` stamps the same platform version the config
  defaults to; the generated hvigor-config declares the plugin version the tool is built against).
- Internal seams (signingConfigs scan, config defaults) extracted for testability — no CLI change.

## [26.0.0-1] — 2026-10-19

### Added

- A VS Code client, maintained in its own repository
  ([hvolt-project/hvolt-vscode](https://github.com/hvolt-project/hvolt-vscode)): registers `.ets`,
  launches `hvolt lsp --stdio`, shows build state in the status bar and exposes Build / Run / Sign /
  Install / Screenshot / Log. The copy that lived in `editor/` was removed once its files and history
  moved there.
- `hvolt lsp --stdio`: a language server for editors.
- Palette commands in the language server: `hvolt.build`, `assemble`, `sign`, `push`, `run`,
  `screenshot`, `log` — each returns `{ok, seconds, tail}` (success taken from the exit code) and
  posts a `window/showMessage` notification.
- Editor features: completion (component attributes, declared object arguments, enum members, ArkTS
  decorators, `$r`/`$rawfile`, module exports together with their import edit), hover, signature help,
  definition, type definition, implementation, references, document highlights, outline, workspace
  symbols, folding, smart selection, rename, quick fixes, semantic highlighting, inlay hints,
  `hvolt.toml` path links and call hierarchy.
- ArkUI blocks are rewritten before the SDK's TypeScript service reads a file (`Column() { … }` becomes
  `Column() ;__arkui(() => { … })`, with a segment map for positions), because the service's parser
  rejects the block syntax and so has no scope inside `build()` — measured 2026-10-19: a completion
  request there returned 1,757 global entries and the local variable was not among them.
- Diagnostics in the language server come from codelinter (`hvolt lint --json`) and from the SDK's
  TypeScript service (type errors), both mapped from the generated trees back to the flat project paths
  and merged per source. Unsaved buffers reach codelinter through an overlay, so what is on screen is
  what gets judged. The rule run is skipped when its inputs are unchanged, because engine startup
  dominates the cost (whole tree 2.94 s, one file 2.78 s, identical content again 0 ms).
- `hvolt lint --scoped` checks only the files an overlay replaced.

### Fixed

- `hvolt.toml` accepts `members` again: the language server reported it as an unknown
  key in `[app]`, although the parser reads it. `type` is no longer offered there — it is
  the toml name of `ModuleInfo.module_type` and belongs to `[module]` only. A test now
  reads `src/config.rs` so the editor's key list cannot drift from the parser again.
- Semantic highlighting no longer reads strings or comments: `'@kit.AbilityKit'` in an
  import was coloured as a decorator plus a property, because the classifier scanned the
  raw line. Strings, template literals and comments are masked before identifiers are
  matched, with the line length kept so positions stay exact.

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
