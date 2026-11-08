# hello-native

hello-world plus one C++/NAPI native module.

```
hello-native/
├── hvolt.toml                 # [dependencies] libhello = { path = "mylib", version = "1.0.0" }
├── src/                       # ArkTS side: import { add } from 'libhello.so'
└── mylib/                     # native module root
    ├── hvolt.toml             # name / type = "native" / types / version
    ├── CMakeLists.txt         # add_library(hello SHARED napi_init.cpp)
    ├── napi_init.cpp          # NAPI entry + __attribute__((constructor)) registration
    └── Index.d.ts             # type declarations for ArkTS
```

Three rules:

1. Declare the native module **only** in `[dependencies]`; never in `app.members` (a member is
   treated as a regular ETS module).
2. `name` in `mylib/hvolt.toml` fixes the artifact name: `hello` → `libhello.so`. The CMake target,
   `nm_modname` and the ArkTS `import` must all use that same name.
3. `types` points at the ArkTS type declaration (`Index.d.ts`). `hvolt build` compiles
   `cpp/` through the SDK toolchain and packs the resulting `<name>.so` into the HAP.

```bash
cd examples/hello-native
hvolt build         # compiles cpp/ through the SDK toolchain and packs the .so into the HAP
hvolt sign --local  # this example has no [app.signing]: sign for a board or emulator
hvolt run           # push and launch the signed package
```

The HAP carries `libs/arm64-v8a/libhello.so` next to `ets/modules.abc`, which is where the
runtime loads a NAPI module from. The ArkTS compiler emits one informational warning that napi
modules are verified by a later SDK version; it is not an error.
