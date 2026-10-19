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

## Real-time diagnostics

Measured on Linux (3-file project), 2026-10-19:

| Fact | Value |
|---|---|
| Full `hvolt lint` (re-translates, always current) | 2.2 s |
| Path-scoped `hvolt lint <file>` | unusable — codelinter resolves paths inside its translated tree |
| Keystroke → published diagnostics, end to end | 2.7 s (2.0 s debounce + 2.2 s lint overlap measured at 2.7 s total) |

Unsaved buffers are linted: `hvolt lint --overlay-file <json>` writes the editor's text
over the mirrored copy in the linted tree, so the engine judges what is on screen.
Verified by putting a syntax error in the buffer only — the disk file stayed clean while
lint reported `@parsing-error/parsing-error | Parsing error: Variable declaration expected. (line 7)`.

`lintOnType` (default on, 2.0 s debounce) refreshes diagnostics after typing stops;
`lintOnSave` keeps the on-save run.

## ArkTS → TS transform

The SDK's parser rejects ArkUI block syntax, and that single limitation explained every
completion/hover gap: inside `build()` the language service had no scope at all (measured:
`coun` → 1,757 global entries, the local `counter` absent).

The server now rewrites the blocks before the service sees the file, keeping every ArkUI
call an expression and turning its block into a helper call:

```ts
Column() {              Column() ;__arkui(()=>{
  Text('x').fontSize(20)    Text('x').fontSize(20)
}.width('100%')         })   .width('100%')
```

`transformArkts` records a segment map, so request positions are converted into the
transformed text and results are converted back; inserted syntax has no original
position. `__arkui` is declared to the program by a shim file the server injects.
Measured after the change: local variables, imports and keywords all complete inside
`build()` (`counter`, `hilog`, `const` first in their prefix), and hover on an attribute
returns the declared signature instead of `any`.

Custom structs are included: every `struct X` declared in the project is treated as an
ArkUI block. Type diagnostics no longer skip ArkUI files — they are computed on the
transformed program and each position is mapped back; findings inside injected syntax are
dropped.

The program pins `lib: ["lib.es2021.d.ts"]`, as the SDK's own ets-loader does. With the
default `lib.es2021.full.d.ts` the DOM's `Text` constructor shadowed the ArkUI `Text`
factory and every component call reported "is not callable" — verified by resolving the
symbol: `lib.dom.d.ts InterfaceDeclaration, VariableDeclaration`. `console` (which lives
in lib.dom) is supplied by the injected shim instead.

## Navigation

`references`, `documentSymbol` and `codeAction` come from the same service as hover and
completion, so they inherit the position mapping: a reference inside transformed syntax
is dropped rather than shown at a wrong place. Measured on a two-use example: two
locations, an outline with `Index` → `constructor`, `build`, `message`, and one quick fix
returned for a known diagnostic code.

## Semantic tokens

The SDK's classifier cannot be used: `getEncodedSemanticClassifications` aborts on ArkUI
files with `Debug Failure. False expression: Classification had non-positive length of 0`,
and the deprecated `getSemanticClassifications` fails the same way; a plain TypeScript
file in the same program classifies fine. So the tokens come from the indexes the server
already builds — components, structs, enums and their members, component attributes,
decorators and the DSL functions (`semantics.ts`, a pure function over the user's text).

Measured on the sample file: 8 tokens — `Entry`/`Component`/`State` as decorators,
`Index` as a class, `Column`/`Text` as classes, `message` as a property, `fontSize` as a
method. Keywords, strings and operators are deliberately not emitted: the grammar already
colours those, and a second opinion there fights the theme.

## Editor-experience features

Four things that separate a language plugin from a usable one, all measured:

- **occurrences** (`documentHighlight`): 3 spans for `message` in the sample file; each
  mapped back, with write/read kinds;
- **inlay hints**: every category is off by default in the service, so the preferences
  turn them on — 4 hints on the sample (`: void`, `value:`, `content:`, `...data:`);
- **auto-import**: completion is asked for module exports, and `completionItem/resolve`
  returns the import edit together with the documentation. The service's own module
  specifier came out as `"../../../../T/…/src/model/Store"` because its view of the paths
  differs from the editor's, so the specifier is recomputed from the entry's target file
  (`"../model/Store"`);
- **completion documentation**: the entry's JSDoc, shown in the resolve step.

## Type diagnostics

Source: the SDK's TypeScript service (`createLanguageService` in the same fork the hover
uses). Measured on the two example modules:

| File | Diagnostics | What happens |
|---|---|---|
| `EntryAbility.ets` (clean) | 0 | published |
| `EntryAbility.ets` + `const broken: number = 'text'` | 1 — "Type 'string' is not assignable to type 'number'" | published (buffer only, file unsaved) |
| `src/pages/Index.ets` (ArkUI `}.modifier(…)` chains) | 4 false parse errors | file skipped entirely, logged |

The skip is a source test (`}` followed by `.name(`), not a per-diagnostic filter: the
fork's parser rejects legal ArkUI chains, so anything it says about such a file is
unreliable. Type errors in files without chains still reach the editor.

Two measured constraints: the check walks every SDK declaration, so it is debounced
(600 ms) with one run at a time — a concurrent run exhausts the heap — and the server
is started with `--max-old-space-size=4096` for room.

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
