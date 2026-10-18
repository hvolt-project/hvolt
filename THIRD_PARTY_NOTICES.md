# Third-party notices

hvolt itself is licensed under **GPL-3.0-or-later** (see [LICENSE](LICENSE)). This file records the
other components that end up in, or are fetched by, a working installation.

## Rust dependencies

All crates pulled through `Cargo.lock` carry permissive licenses compatible with GPL-3.0. Versions
are pinned in `Cargo.lock`; the license families are:

| License | Crates (selection) |
|---|---|
| MIT OR Apache-2.0 | clap, serde, serde_json, toml, ureq, sha2, base64, rand, flate2, indexmap, url, idna, log, once_cell, syn, quote, proc-macro2, libc, regex-lite, … (the large majority) |
| MIT | generic-array, simd-adler32, strsim, synstructure, winnow, zip, zmij |
| Apache-2.0 AND ISC | ring |
| Apache-2.0 OR ISC OR MIT | rustls |
| ISC | rustls-webpki, untrusted |
| BSD-3-Clause | subtle |
| BSD-2-Clause OR Apache-2.0 OR MIT | zerocopy, zerocopy-derive |
| Unicode-3.0 | icu_* family, zerovec*, zerofrom*, zerotrie, yoke*, tinystr, writeable, litemap, potential_utf |
| Unlicense OR MIT | memchr |
| 0BSD OR MIT OR Apache-2.0 | adler2 |
| MIT OR Zlib OR Apache-2.0 | miniz_oxide |
| Apache-2.0 | zopfli, zeroize |
| CDLA-Permissive-2.0 | webpki-roots (CA root data, not code) |
| MIT⁄Apache-2.0 (build scripts) | cc, find-msvc-tools, windows-targets family |

Full license texts ship with each crate in the Cargo registry (`~/.cargo/registry/src/`).

## Assets in this repository

- The project icons under `templates/media/` and `examples/*/resources/base/media/`
  (`background.png`, `foreground.png`, `startIcon.png`, `layered_image.json`) are original works
  created for this project. An earlier revision contained image files byte-identical to assets
  bundled with DevEco Studio; those were replaced and are not redistributed here.

## Fetched at runtime (not redistributed)

- **OpenHarmony public test signing materials** — `hap-sign-tool.jar` inputs from
  [openharmony/developtools_hapsigner](https://gitee.com/openharmony/developtools_hapsigner)
  (`dist/`), licensed **Apache-2.0**. hvolt downloads them on demand under `~/.hvolt/` and pins
  their SHA-256. They identify the *public test* chain only — valid for development boards,
  emulators and source-built ROMs, not for retail devices.
- **HarmonyOS SDK components** — downloaded from Huawei's mirrors by `hvolt sdk download`; use is
  governed by Huawei's HarmonyOS SDK terms, accepted when you obtain the SDK.
- **hvigor, ohpm** — fetched from the npm registry / Huawei mirrors by `hvolt tools install`.
- **Code Linter payload** — taken from a DevEco Studio installation **you supply** via
  `hvolt tools install codelinter --from <dir>`. hvolt does not bundle, mirror or extract it.
- **Emulator binary and system images** — not distributed with hvolt; the official binaries come
  with DevEco Studio.

## Trademarks

"HarmonyOS", "DevEco Studio", "AppGallery Connect" and "Huawei" are trademarks of Huawei
Technologies Co., Ltd. This project is independent and is not affiliated with, endorsed or
sponsored by Huawei; the names are used only to describe compatibility and interoperability.
