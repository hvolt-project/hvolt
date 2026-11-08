# Building, Signing and Installing Across Hosts

hvolt's build, sign and install steps do not have to run on the same machine. This document
describes the roles, the two signing chains, and the pitfalls that were measured on real
projects. It targets the general case: you may run everything on one host, or split it across
several.

## Roles (when you split the work)

| Role | What it needs | Responsibility |
|---|---|---|
| Build host | Rust toolchain, SDK compilers | `hvolt build` (produces the signed HAP) |
| Sign host | AppGallery Connect login state (see below) | `hvolt sign` (exchanges CSR for cert + profile) |
| Device host | the physical device (USB or wireless) | `hvolt push` / shell over the device link |

The two constraints that usually force a split:

- **Signing must reach AppGallery Connect.** Any host with a valid AGC login can sign.
- **Installing must reach the device.** Only a host that can reach the device (`hvolt device
  list`, or a `hvolt device remote` target) can push.

Nothing in hvolt requires the same host to do both. When the device is not attached to the
signing host, hand the UDID over explicitly:

```bash
HVOLT_DEVICE_UDID=$UDID hvolt sign            # on the sign host
```

## When the build host cannot reach the device

A build host is often on a different subnet than the device, so `hvolt push` there times out
even though the package is ready. Relay the signed HAP to a host that can reach the device
instead of rebuilding or re-signing elsewhere (measured 2026-11-08):

```bash
# 1 build + sign on the build host
hvolt build                                   # → output/<name>-signed.hap

# 2 copy to the device host (scp target must be a Windows-style path on a Windows host)
scp output/<name>-signed.hap user@device-host:C:/tmp/

# 3 install from the device host — external files need --hap
hvolt push --hap C:/tmp/<name>-signed.hap -d <ip:port>

# 4 launch an already-installed package without rebuilding it
hvolt exec <ip:port> --shell "aa start -a EntryAbility -b <bundle>"
```

`hvolt run` re-reads `hvolt.toml` and rebuilds, so it cannot be used to just launch an installed
package; `hvolt exec … --shell` sends the command over the device link instead. Wireless targets
do not appear in `hvolt device list` (that lists USB only) — `hvolt device connect <ip:port>`
verifies reachability and records the address in the config.

## Two signing chains — pick the right one or the HAP will not install

| Chain | Command | Trusted by |
|---|---|---|
| OpenHarmony localSign | `hvolt sign --local` | Dev boards / emulators / self-built ROMs. **Retail devices refuse it**: `code:9568257 error: fail to verify pkcs7 file` |
| AppGallery Connect | `hvolt sign` (without `--local`) | Certificates and profiles issued to a Huawei developer account; retail devices accept these |

The two identity domains are not interchangeable: AGC-signed identities are rejected by
localSign-trusting environments and vice versa.

> Note: a binary built without the AGC backend (public build) explains this when you run
> `hvolt sign` or `hvolt agc` — it says the backend is missing instead of pretending.

## Signing happens in two steps — don't stop after `sign`

`hvolt sign` does three things: generate the keystore and CSR, exchange the CSR for a cert and
provisioning profile at AGC, and write `[app.signing.default]` into `hvolt.toml`. **It does not
sign the HAP.** The HAP is signed on the next `hvolt build` (the output shows
`Signing HAP... ✓ <bundle>-signed.hap`).

```bash
hvolt build                 # unsigned HAP
hvolt sign                  # cert + profile + config (hand the UDID over if the device is elsewhere)
hvolt build                 # ← this one produces the signed HAP
```

## Pitfalls (all measured)

- **`runtime_os` and `device_types` must match the installed SDK.** On a host without the HMS
  components, `runtime_os = "HarmonyOS"` fails with `00303168 SDK component missing`. Switch to
  `"OpenHarmony"` and use only the device types that SDK provides — `phone` may be rejected.
  Run `hvolt doctor` first; it spells this out.
- **AGC login expires.** `hvolt agc login` prints a local `apply?port=<port>` URL to open in a
  browser to complete the callback. When the refresh token expires, signing fails with
  `jwToken check failed … 401` — just log in again.
- **`~/.hvolt` may be unwritable** (sandbox or permissions). Point `HOME` at a writable copy
  inside your workspace; login state and signing materials then land there.
- **A generated keystore password must not be pure hexadecimal.** `hvolt build` treats a
  password of 32+ hex-only characters as a DevEco-encoded password and refuses to sign, while
  early AGC generators produced exactly 64-hex passwords — the tool rejected its own output.
  The AGC generator now uses a non-hex alphabet (since 26.0.0-3). Workaround if you hit an old
  password: repackage the keystore with `keytool -importkeystore` using a non-hex password and
  update `hvolt.toml` to match.
- **Wireless debugging drops.** When `hvolt device list` shows no target, reconnect first:
  `hvolt device connect <ip:port>`.
- **`hvolt sign` reports `No device UDID found`** when no device is visible — pass
  `HVOLT_DEVICE_UDID` as shown above.
- **`hvolt push`'s positional argument is not a file path.** It is treated as a project
  artifact name and looked up under `output/` + `build/`. Pass an arbitrary file with
  `--hap <FILE>`; an unsigned package is rejected before it reaches the device rather than
  failing at install time.
- **`local.properties` (sdk.dir)**: hvolt no longer needs it: the native chain reads the SDK path from config
  resolves the SDK from that file as well as `DEVECO_SDK_HOME`; without it, module tasks abort
  with `00303208 Unable to find 'sdk.dir'`. DevEco Studio writes the same file.
- **Verify the release build you actually ship.** `hvolt agc whoami` returning an account means
  the AGC backend is present; "this build has no AppGallery Connect backend" means it is the
  public build. Check this on the artifact host rather than assuming.