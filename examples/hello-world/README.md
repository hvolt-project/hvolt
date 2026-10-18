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
hvolt assemble      # translate only: flat tree → standard HarmonyOS project under build/
hvolt build         # translate + hvigorw build + sign
hvolt run           # build, then push and launch
```

`hvolt init my-app` produces exactly this layout. The output of `hvolt assemble` (`build/`) is a
complete HarmonyOS project: running `hvigorw assembleHap` on it yields `BUILD SUCCESSFUL`. Without a
signing config it produces `*-unsigned.hap`, which is separate from the signing step of
`hvolt build`.
