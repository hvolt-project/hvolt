# LSP for hvolt — design

Goal: give VS Code (and any LSP client) real ArkTS editing support for hvolt projects, without
requiring DevEco Studio and without the user abandoning hvolt's flat project layout.

## What already exists (measured, 2026-10-19)

The SDK hvolt downloads ships a complete language service; nothing about ArkTS semantics has to be
reimplemented.

| Fact | Value |
|---|---|
| TypeScript build inside the SDK | `<sdk>/ets/build-tools/ets-loader/node_modules/typescript`, version **4.9.5-r4** |
| Fork capability | registers `.ets` (99 references), exposes `StructDeclaration`; `navtree` reports `{"text":"Index","kind":"struct"}` |
| Language service entry points | `lib/tsserver.js` (separate process), `lib/tsserverlibrary.js` (in-process API) |
| Declarations | `ets/api` 532 `.d.ts`, `ets/kits` 47, `ets/component` (ArkUI), `ets/arkts` (`@arkts.*`) |
| ArkTS checker/linter | `ets/build-tools/ets-loader/lib/ets_checker.js`, `lib/do_arkTS_linter.js` |
| Present in a plain `hvolt sdk download`? | yes — verified on the GPU server's `~/.hvolt/sdk/default` |
| `typescript-language-server` | **cannot** drive this fork: it frames with `Content-Length`, the fork speaks JSON lines (zero responses) |
| In-process `createLanguageService` | works: `Column` → `const Column: ColumnInterface (options?: ColumnOptions): ColumnAttribute`, `@State` → `const State: PropertyDecorator`, 940 completion items |

## Architecture

```
VS Code extension (thin)                     hvolt (Rust CLI)
        │  spawn "hvolt lsp --stdio"               │
        ▼                                          ▼
┌───────────────────────────── hvolt-lsp (Node) ─────────────────────────────┐
│ ① host      project model: hvolt.toml → modules, abilities, routes, deps   │
│             file snapshots of the *flat* tree, plus SDK declarations        │
│ ② semantics SDK fork's createLanguageService (tsserverlibrary.js)           │
│             hover · completion · definition · signatureHelp · rename        │
│             (its own diagnostics are NOT published — see "Diagnostics")     │
│ ③ diagnostics from the real toolchain                                       │
│             hvigor build errors · codelinter (hvolt lint --json)            │
│             app_check_tool (hvolt check)                                    │
│ ④ commands  build · sign · push · screenshot · log (executeCommand)         │
└────────────────────────────────────────────────────────────────────────────┘
```

The decisive property: **the language service does not care about directory layout.** It needs `.ets`
files and the SDK declarations; the flat tree can be handed over as-is. That removes the need for an
on-disk "standard view", and with it the Windows symlink problem (`hvolt ide sync` is only needed for
editors that insist on their own language client).

## Phases

| Phase | Scope | Done when |
|---|---|---|
| **M1** ✅ | `hvolt lsp --stdio`: host layer + fork semantics + document sync | done — `RelativeContainer` hovers as `RelativeContainerInterface`, `Text` defines into `component/text.d.ts`, 940 completions, ~0.2 s to initialize |
| **M2** ✅ | diagnostics from the toolchain only, positions mapped back to real files | done — codelinter findings (54 defects on a demo project) and a compile error (`pages/Index.ets L8:12 — Cannot find name 'noSuchVariable'`, matching hvigor's `Index.ets:8:12`) both arrive on flat paths; lint and build results are merged per source, so one does not clear the other |
| **M3** ✅ | hvolt intelligence. Key completion per section; route completion from the module's `routes`; validation (unknown keys, non-array `device_types`, missing route files); dependency navigation (`lib = { path = "mylib" }` → `mylib/hvolt.toml`); palette commands `hvolt.build` / `assemble` / `sign` / `push` / `run` / `screenshot` / `log`, each returning `{ok, seconds, tail}` and a `window/showMessage` notification | done — measured on the GPU server (`hvolt build → ok (0.9s)`) and on a machine without hvigor (`failed`, from the exit code, not from grepping text) |
| **M4** ✅ | VS Code extension in `editor/vscode`: `.ets` language, launches `hvolt lsp --stdio`, status bar, Build / Run / Sign / Install / Screenshot / Log, settings for binary, SDK, device, lint-on-save | done — `@vscode/vsce package` produces a 128 KB `.vsix` (10 files); a manifest test keeps contributed commands, language id and the launched server in step |

## Protocol log

`--log-file <path>`, `HVOLT_LSP_LOG` or the editor setting `hvolt.logFile` makes the server
append one line per `initialize` / `didOpen` / `hover` / `completion`. It exists because a
misbehaving client is otherwise invisible: with it, "the editor never opened the file" and
"the server answered wrongly" become distinguishable. Measured output from a real VS Code
window: `didOpen file:///…/src/pages/Index.ets (ets, 24 lines)`.

## Protocol contract

- `hvolt lsp --stdio` speaks LSP 3.17 over stdio; the extension only spawns it.
- `hvolt lsp --version` reports the hvolt version and the SDK the language service came from.
- The server locates the SDK through the same resolution hvolt already uses (`sdk_path`, the managed
  store, then DevEco). If no SDK is present it still starts and reports a single diagnostic telling
  the user to run `hvolt sdk download`.

## Risks and open questions

1. **The fork's own diagnostics are wrong for a legal ArkUI idiom** (measured 2026-10-19):

   ```
   Column() { Text('x') }.width('100%')   → 1 diagnostic: Cannot find name 'width'
   Column().width('100%')                 → 0 diagnostics
   ```

   Block-then-chain (`Block { … }.method()`) is valid ArkUI and is rewritten by `ets-loader`'s
   transformer; the bare checker sees the `.method()` after a closing brace as a new statement. The
   file is parsed as ETS (`ScriptKind` 8), so this is not a configuration mistake. Consequence for the
   design: the language service is used for hover, completion, definition, signature help and rename,
   **not** as a diagnostics source; diagnostics come from the real toolchain (`hvigor`, codelinter,
   `app_check_tool`), which is also what makes an editor line match a build failure exactly.
2. **Cost of loading declarations** — 5200+ files; measure time-to-first-hover and memory, and consider
   limiting to `api/` + `component/` + only the kits an import actually needs.
3. **Licensing** — the fork is invoked from the user's own SDK, never redistributed with hvolt; same
   posture as the codelinter payload. The Node server itself must ship without any SDK content.
4. **Node requirement** — the server runs on Node (hvolt already needs it for hvigorw); pin a minimum
   version and fail with a clear message.
5. **codelinter availability** — M2's lint diagnostics depend on the payload the user installs with
   `hvolt tools install codelinter --from <dir>`; the server must degrade silently when absent.

## Rejected alternatives

| Alternative | Why not |
|---|---|
| Point the editor at `build/` | generated tree is rewritten every build; edits and definitions land on copies |
| Copy sources into a standard-shaped directory | edits are lost unless written back; fragile |
| Symlink the flat tree into a standard view | Windows needs developer mode; solves a problem the language service does not have |
| Reuse `typescript-language-server` with the SDK fork | protocol mismatch (Content-Length vs JSON lines) |
| Implement ArkTS semantics in Rust | would require reimplementing the TypeScript checker |
