# hello-world

Smallest hvolt project: one module, one UIAbility, one page.

```
hello-world/
├── hvolt.toml              # application config: bundle, version, members, dependencies
├── src/
│   ├── hvolt.toml          # module config: entry ability, routes, device types
│   ├── EntryAbility.ets
│   └── pages/Index.ets     # @Entry page
└── resources/base/         # icons and element/{string,color,float}.json
```

```bash
cd examples/hello-world
hvolt build         # compile ArkTS, pack into build/ (signs when configured)
hvolt sign --local  # this example has no [app.signing]: sign for a board or emulator
hvolt run           # push and launch the signed package
```

`hvolt init my-app` produces exactly this layout. `hvolt build` reads only `hvolt.toml` and the
sources: it does not generate a DevEco project under `build/`, so `hvigorw` has nothing to open
there. Without a signing config it produces `build/*-unsigned.hap`.
