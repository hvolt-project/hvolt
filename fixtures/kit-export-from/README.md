# kit-export-from

`process_kit_import.js` expands a re-export that names a Kit before the file is printed:

- `ExportSpecifierKitInfo.transform` groups the clause by the module each Kit symbol comes from
  and writes one `export { … } from "<module>";` per group. The exported name is the config's
  `bindings` field, so `export { curves }` from a symbol with `bindings: "default"` becomes
  `export { default as curves } from "@native:ohos.curves";`.
- `ExportStarKitInfo.transform` writes one `export * from "<module>";` per distinct module of
  the Kit, in the config file's order, and reports a WARN that every sub-module loads at
  runtime. `@system.` sources are folded into the `@ohos:` namespace (`@ohos:package`).

The re-exports are written above the struct, so the fixture also pins their position: the
reference keeps them where the source wrote them, before the class.
