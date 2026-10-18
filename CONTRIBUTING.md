# Contributing to hvolt

Thanks for taking the time. This document covers what a change needs before it can be merged.

Releases are cut from a tag by CI — see [RELEASING.md](RELEASING.md).

## Before you start

- **Bug reports and design questions**: open an issue first. Small fixes (typos, docs, obvious
  bugs) can go straight to a pull request.
- **Larger changes** (new commands, changes to the project layout, changes to generated
  HarmonyOS output): open an issue so the approach can be agreed before you write code. The CLI
  surface and the `hvolt.toml` schema are public interfaces — a change there needs a reason and,
  once shipped, a note in `CHANGELOG.md`.

## Setting up

Requirements: Rust 1.85 or newer (the crate pins `rust-version`). `cargo build` is enough for most
work; only end-to-end runs need a HarmonyOS SDK and JDK 17+:

```bash
cargo build
cargo test
```

For an end-to-end run without DevEco Studio:

```bash
cd examples/hello-world
hvolt sdk download 26.0.0.38     # or hvolt sdk set /path/to/sdk
hvolt tools install hvigor
hvolt tools install ohpm
hvolt build
```

`sdk download`, `tools install` and `build` write under `~/.hvolt/` and the project directory; see
`hvolt doctor` for what is detected and what is missing.

## What a pull request must pass

```bash
cargo fmt --all --check                      # empty output
cargo clippy --all-targets -- -D warnings    # "Finished", no warnings
cargo test                                   # all passing
```

- **Behaviour changes need a test.** Unit tests live next to the code they cover.
- **Measured claims need the measurement.** If a comment or commit says the engine, SDK or device
  behaves in a particular way, say how it was observed (command, output, date). Claims that were
  argued rather than measured are the ones that turned out wrong.
- **No compatibility shells.** hvolt is a binary crate: there is no downstream API to keep stable,
  so dead code is removed rather than kept "for safety".
- **No swallowed errors.** `let _ = something()?;`-style discards need a stated reason; user-visible
  failures go through `eprintln!("✗ …")` and a non-zero exit code.
- **Help text and comments must match reality.** If you change a command or a flag, update the
  `--help` text and any comment that names it in the same commit.
- **Keep the diff focused.** One topic per pull request; formatting-only changes go in their own
  commit.

## Style

Code is plain Rust with `rustfmt` defaults (`max_width = 100`). Comments are technical: no first
person, no internal work-item codes, no metaphor. Prefer a measured fact plus a date over
speculation.

For the generated HarmonyOS side, the rules are in the README section
[Project layout](README.md#project-layout); native modules are demonstrated in
[`examples/hello-native`](examples/hello-native).

## What we cannot merge

- A client for AppGallery Connect endpoints. (The private overlay also obfuscates its string
  constants and relies on release settings that strip symbols; such code is out of scope here.) Remote signing depends on a backend that is
  deliberately kept out of this repository (see the README section "Signing and AGC"); the public
  tree carries only the local-signing path and a facade that reports the backend as absent.
- Code that redistributes DevEco Studio artifacts. hvolt installs the Code Linter payload only from
  a directory the user supplies (`hvolt tools install codelinter --from <dir>`); extracting it from
  an installer is deliberately out of scope, as is bundling Huawei binaries.
- Changes that secret-scan or upload user data. hvolt keeps state under `~/.hvolt/` and does not
  transmit anything except the SDK/tool downloads it is asked to perform.

## Commits and licensing

Write commit messages in the imperative mood, explaining *why* the change is needed. By
contributing you agree that your work is licensed under GPL-3.0-or-later, the license of this
project (inbound = outbound).
