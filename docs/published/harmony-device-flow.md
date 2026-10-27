# Building, Signing and Installing Across Hosts

hvolt's build, sign and install steps do not have to run on the same machine. This document
describes the roles, the two signing chains, and the pitfalls that were measured on real
projects. It targets the general case: you may run everything on one host, or split it across
several.

## Roles (when you split the work)

| Role | What it needs | Responsibility |
|---|---|---|
| Build host | Rust toolchain, SDK, hvigor/ohpm/node/jdk | `hvolt build` (produces the unsigned HAP) |
| Sign host | AppGallery Connect login state (see below) | `hvolt sign` (exchanges CSR for cert + profile) |
| Device host | hdc + the physical device (USB or wireless) | `hvolt push` / `hdc install` / `hdc shell` |

The two constraints that usually force a split:

- **Signing must reach AppGallery Connect.** Any host with a valid AGC login can sign.
- **Installing must reach the device.** Only the host that can see the device (hdc `list
  targets`) can push.

Nothing in hvolt requires the same host to do both. When the device is not attached to the
signing host, hand the UDID over explicitly:

```bash
UDID=$(hdc shell bm get --udid | tail -1)     # on the device host
HVOLT_DEVICE_UDID=$UDID hvolt sign            # on the sign host
```

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
- **Wireless debugging drops.** When `hdc list targets` only shows `COM*` entries and install
  reports `need connect-key`, reconnect first: `hdc tconn <ip:port>`.
- **`hvolt sign` reports `No device UDID found`** when no device is visible — pass
  `HVOLT_DEVICE_UDID` as shown above.
- **`local.properties` (sdk.dir)**: hvolt writes it during `run_assemble` because hvigor
  resolves the SDK from that file as well as `DEVECO_SDK_HOME`; without it, module tasks abort
  with `00303208 Unable to find 'sdk.dir'`. DevEco Studio writes the same file.
- **Verify the release build you actually ship.** `hvolt agc whoami` returning an account means
  the AGC backend is present; "this build has no AppGallery Connect backend" means it is the
  public build. Check this on the artifact host rather than assuming.