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
│             semantic/syntactic diagnostics                                  │
│ ③ aggregate diagnostics from the real toolchain                             │
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
| **M1** | `hvolt lsp --stdio`: host layer + fork semantics + document sync | a real `.ets` file in `examples/hello-world` gets hover, completion and definition in VS Code |
| **M2** | diagnostics aggregation, positions mapped back to real files | a deliberate build error shows as an inline diagnostic identical to `hvolt build`'s message |
| **M3** | hvolt intelligence: `hvolt.toml` validation/completion, route targets for `router.pushUrl`, cross-module definition, commands | editing `hvolt.toml` gives completions; `pushUrl('pages/…')` completes from routes; build/run available from the palette |
| **M4** | VS Code extension published (`.ets` association, status bar, device picker) | installable extension drives M1–M3 |

## Protocol contract

- `hvolt lsp --stdio` speaks LSP 3.17 over stdio; the extension only spawns it.
- `hvolt lsp --version` reports the hvolt version and the SDK the language service came from.
- The server locates the SDK through the same resolution hvolt already uses (`sdk_path`, the managed
  store, then DevEco). If no SDK is present it still starts and reports a single diagnostic telling
  the user to run `hvolt sdk download`.

## Risks and open questions

1. **False diagnostics** — one semantic error surfaced in the spike (`Cannot find name 'width'` on a
   chained `Column().width()`); must be understood before M2 ships, since a noisy server is worse than
   none. Cause is likely snapshot versioning or missing `ets-loader` compiler options.
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
