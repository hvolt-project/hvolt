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

1. Update the version in `Cargo.toml`, `npm/package.json` and every `npm/platforms/*/package.json`
   (they must match) and add a `CHANGELOG.md` entry under the new version.
2. Commit, push to `main`, and let CI finish.
3. Tag and push:

   ```bash
   git tag -a v0.1.0 -m "hvolt 0.1.0"
   git push origin v0.1.0
   ```

4. The workflow then:
   - runs `cargo fmt --check`, `cargo clippy -D warnings` and `cargo test` on Linux,
   - builds `darwin-arm64`, `linux-x64`, `win32-x64`,
   - verifies the tag equals `npm/package.json`'s version,
   - publishes the three platform packages, then the launcher (both with provenance),
   - writes `SHA256SUMS.txt` over the archives,
   - creates the GitHub release with the three archives plus `SHA256SUMS.txt` attached and the
     changelog as notes.

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
