# builder-cross-file

A `@Builder function` declared in another file and called from `pages/Index.ets` is emitted as
`pageBuilder.bind(this)(…)` — the call site carries no component frame and the builder's own
body belongs to the other module.

This fixture needs a second source file, so it does not fit the one-`input.ets` shape of the
other fixtures:

- `input.ets` → `project/src/pages/Index.ets` (copied by `new-fixture.sh`);
- `project/src/pages/Shared.ets` holds `@Entry struct SharedPage` and the exported builder
  `pageBuilder`;
- `project/src/hvolt.toml` lists both pages in `routes`, which is what makes `emit-layout.py`
  copy `Shared.ets` into the hvigor layout.

`new-fixture.sh` only overwrites the template files, so it keeps `Shared.ets`; it does rewrite
`project/src/hvolt.toml` from the template, so re-running it needs the route to be added back
(`routes = ["pages/Index.ets", "pages/Shared.ets"]`).

The comparison covers `pages/Index.ts` only; the builder's declaration lives in
`pages/Shared.ts` and is not part of the golden.