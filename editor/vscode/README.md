# hvolt for VS Code

ArkTS support for [hvolt](https://github.com/hvolt-project/hvolt) projects.

- Syntax highlighting: TypeScript comes from VS Code's built-in `source.ts` grammar; the ArkTS additions
  (decorators such as `@Entry`/`@State`, the `struct` keyword, `$r(...)` resources, ArkUI components)
  are defined in `syntaxes/ets.tmLanguage.json`.
- Language server: hover, completion, definition, signature help and rename, with semantics from
  the TypeScript build inside the installed SDK — no DevEco Studio needed.
- Diagnostics from the real toolchain: codelinter on save, plus build errors mapped onto the
  files that caused them.
- `hvolt.toml`: key completion per section, route completion inside `router.pushUrl(...)`,
  validation (unknown keys, missing route files), and navigation from a dependency to its module.
- Commands: Build, Build+install+launch, Sign, Install on device, Screenshot, Fetch log.

Requires `hvolt` on `PATH` (or set `hvolt.binaryPath`) and an installed SDK
(`hvolt sdk download <version>`).

The server can also be driven by any other LSP client: `hvolt lsp --stdio`.
