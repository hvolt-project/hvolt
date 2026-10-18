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
3. `types` points at the ArkTS type declaration (`Index.d.ts`). `assemble` copies it to
   `src/main/cpp/types/<dependency>/` and adds
   `"libhello.so": "file:./src/main/cpp/types/libhello"` to the module's `oh-package.json5`.

```bash
cd examples/hello-native
hvolt assemble      # produces src/main/cpp/{CMakeLists.txt,napi_init.cpp,types/libhello/*}
hvolt build         # includes the C++ build (CMake + NDK)
```

Measured on this tree: after `assemble`, `hvigorw assembleHap` on `build/` reports `BUILD SUCCESSFUL`
and produces `libhello.so` (arm64-v8a) plus `src-default-unsigned.hap`. The ArkTS compiler emits one
informational warning that napi modules are verified by a later SDK version; it is not an error.
