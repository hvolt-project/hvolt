# npm packaging

`hvolt` ships on npm as one small launcher package plus one package per platform. The launcher
resolves the platform package and execs its binary, so `npm install -g @hvolt/cli` gives you a working
`hvolt` on macOS, Linux and Windows without a Rust toolchain.

| Package | Contents |
|---|---|
| `hvolt` | `bin/hvolt.js` launcher; lists the platform packages as `optionalDependencies` |
| `@hvolt/cli-linux-x64` | prebuilt binary, `os`/`cpu` constrained |
| `@hvolt/cli-darwin-arm64` | prebuilt binary |
| `@hvolt/cli-win32-x64` | prebuilt `hvolt.exe` |

npm installs only the package whose `os`/`cpu` match, so a global install pulls one binary
(5–7 MB compressed to 2–3 MB).

## Building the packages

```bash
# 1. produce the binaries
cargo build --release                                             # host
cargo zigbuild --release --target x86_64-pc-windows-gnu           # Windows
cargo zigbuild --release --target x86_64-unknown-linux-gnu        # Linux (or build on Linux)
# ... the remaining triples for the matrix you want to publish

# 2. copy them into the platform packages (also accepts prebuilt/<key>/ drops)
node npm/stage.mjs

# 3. pack (repeat per package)
cd npm && npm pack
cd npm/platforms/<key> && npm pack
```

`npm/stage.mjs` refuses to run when a platform has no binary, so a partial matrix cannot be
published by accident. Binaries that cargo does not produce locally can be dropped into
`npm/prebuilt/<key>/hvolt[.exe]`.

## Publishing

```bash
npm publish --access public                     # from npm/            → launcher
cd platforms/<key> && npm publish --access public   # one per platform package
```

Order matters: publish the platform packages first, the launcher last, so the launcher's
`optionalDependencies` resolve immediately. Keep the version identical across all six packages —
they are released together.

Packages are published under the `@hvolt` scope as `@hvolt/cli` plus `@hvolt/cli-<key>`; if you fork
this and publish your own scope, update
the names in `npm/package.json`, `npm/bin/hvolt.js` (`SUPPORTED` prefix) and the platform
`package.json` files.

## What the npm packages do not include

- The Rust sources — that is `cargo install --path .` or a published crate.
- The AppGallery Connect backend. `hvolt sign --local` works; remote signing reports that the
  backend is absent (see the README section "Signing and AGC").
