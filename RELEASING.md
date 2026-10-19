# Releasing hvolt

Releases are produced by [`.github/workflows/release.yml`](.github/workflows/release.yml) from a tag.
Nothing is built on a developer machine, so a release is reproducible from the tag alone.

## One-time setup

1. **npm organisation** — create the `hvolt` organisation on npm. The workflow publishes
   `@hvolt/cli` plus `@hvolt/cli-darwin-arm64`, `@hvolt/cli-linux-x64`, `@hvolt/cli-win32-x64`.
2. **npm token** — create an automation token with publish rights for the `@hvolt` scope and store it
   as the repository secret `NPM_TOKEN` (Settings → Secrets and variables → Actions).
3. **GitHub security** — enable *Private vulnerability reporting* so the advisory link in
   [`SECURITY.md`](SECURITY.md) resolves.

## Release procedure

Publishing is automatic once a version is new; nobody pushes a tag by hand.

1. Bump the version in `Cargo.toml` and `npm/package.json` (they must match) and add a
   `CHANGELOG.md` entry, in the pull request that carries the change.
2. Merge. `.github/workflows/release.yml` runs on every push to `main` and asks one
   question: does `v<version>` exist yet?

   - **no** → it builds all three targets, runs the gates, publishes to npm (when
     `NPM_PUBLISH` is `true`), creates the tag itself and attaches the archives plus
     `SHA256SUMS.txt` to the release;
   - **yes** → it stops after the version check, so ordinary merges cost nothing.

3. Nothing else. Re-running is safe: the publish steps skip a package that already exists
   and upload assets with `--clobber`.

A tag push (`git push origin v26.0.0-1`) still works and is the way to re-drive a release
after fixing something in CI; the guard then takes the version from the tag and refuses if
it disagrees with the manifests.

### What this does not decide

The version bump itself. That is the one deliberate act left: the number says which SDK
platform the release claims to support (`26.0.0` = platform 26.0.0), so it changes when the
project decides to support a new platform or to ship a fix — not on every merge.

`install.sh` and `install.ps1` read those assets, so a release is installable as soon as it exists.

`workflow_dispatch` runs the build and the gates only (`dry_run` defaults to true), which is useful to
check a matrix change without publishing.

## Version policy

The version is the SDK version plus a release counter, written as a semver prerelease because Cargo
and the npm registry both reject four-part versions:

| Release | Version | Tag |
|---|---|---|
| first for SDK 26.0.0 | `26.0.0-0` | `v26.0.0-0` |
| next fix, same SDK | `26.0.0-1` | `v26.0.0-1` |
| SDK moves to 26.1.0 | `26.1.0-0` | `v26.1.0-0` |
| SDK moves to 27.0.0 | `27.0.0-0` | `v27.0.0-0` |

The five manifests are rewritten from the tag by the workflow, so a release needs no manual version
edits. Prerelease versions require `npm publish --tag latest`, which the workflow passes; `latest`
therefore always points at the newest release and a plain `npm install -g @hvolt/cli` does the right
thing. The known cost of this scheme is that semver ranges (`^26`) and dependency bots skip
prereleases — installs must use no range or an exact version.

Only **one** dist-tag is maintained (`latest`). Per-SDK-line tags are deliberately not used: a project
pins its SDK in `hvolt.toml`, and hvolt downloads whichever SDK a project asks for, so an install is
never bound to the line it was made for.

### Backward compatibility promise

A new release must still build projects that declare an *older* `sdk_version`. If that ever stops
being true, the old line needs a maintenance branch (`release/26`) and a matching dist-tag — that is
the trigger for adding them, not SDK numbering by itself.

## Platform matrix

Only platforms built by the workflow are published. `npm/platforms/` also holds `darwin-x64` and
`linux-arm64` package manifests for later use; they are not in `optionalDependencies` and are not
published until the matrix covers them.
