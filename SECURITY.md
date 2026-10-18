# Security policy

## Supported versions

Security fixes target the latest `0.1.x` release. Until 1.0 the CLI surface can change, so please
test against the newest release before reporting.

## Reporting a vulnerability

Report privately through GitHub's advisory form:

<https://github.com/hvolt-project/hvolt/security/advisories/new>

Please do not open a public issue for a security problem. Include:

- the hvolt version (`hvolt --version`) and how it was installed (source / prebuilt),
- the operating system and the command that triggers the problem,
- the output, with any token, password, keystore or profile content removed.

You can expect an initial response within a few days. A fix and a coordinated disclosure date will
be agreed in the advisory.

## What hvolt touches on your machine

Useful context when judging an issue:

- **State** lives under `~/.hvolt/` (`config.json`, `auth.json`, `sdk/`, `tools/`, `lint/`).
  `auth.json` holds AppGallery Connect tokens; treat it as a credential.
- **Signing material** (`.p12`, `.cer`, `.p7b`) and their passwords live in the project directory
  and in `hvolt.toml`. hvolt does not upload them; signing happens locally unless you use the
  AppGallery Connect remote signing flow, which sends them to Huawei as that flow requires.
- **Downloads**: SDK components and CLI tools come from Huawei mirrors and the npm registry. hvolt
  verifies archives it can, but treat downloaded tool versions as what you asked for.
- **`hvolt lint`** executes a third-party JavaScript bundle from DevEco Studio with Node.js, over
  the project's translated sources. Supply the payload from a DevEco installation you trust.
- **No telemetry.** hvolt does not report usage anywhere.
