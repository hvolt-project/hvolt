#!/usr/bin/env node
// Copies prebuilt binaries into npm/platforms/<key>/bin/ so each platform
// package can be packed. Sources, in order: npm/prebuilt/<key>/<name>, then the
// matching cargo target directory (cross builds included).
import { copyFileSync, chmodSync, existsSync, mkdirSync, statSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, "..");

const TARGETS = {
  "darwin-arm64": { triple: "aarch64-apple-darwin", bin: "hvolt" },
  "darwin-x64": { triple: "x86_64-apple-darwin", bin: "hvolt" },
  "linux-x64": { triple: "x86_64-unknown-linux-gnu", bin: "hvolt" },
  "linux-arm64": { triple: "aarch64-unknown-linux-gnu", bin: "hvolt" },
  "win32-x64": { triple: "x86_64-pc-windows-gnu", bin: "hvolt.exe" },
};

const missing = [];
for (const [key, { triple, bin }] of Object.entries(TARGETS)) {
  const dest = join(here, "platforms", key, "bin", bin);
  mkdirSync(dirname(dest), { recursive: true });
  const candidates = [
    join(here, "prebuilt", key, bin),
    join(root, "target", triple, "release", bin),
  ];
  // Host build: only for the platform/arch this machine actually is.
  if (key === `${process.platform}-${process.arch}`) {
    candidates.push(join(root, "target", "release", bin));
  }

  const src = candidates.find((p) => existsSync(p));
  if (!src) {
    missing.push(`${key} (expected ${bin})`);
    continue;
  }
  copyFileSync(src, dest);
  if (bin === "hvolt") chmodSync(dest, 0o755);
  console.log(`  ${key.padEnd(13)} ${(statSync(dest).size / 1e6).toFixed(1)} MB  ← ${src.replace(root + "/", "")}`);
}

if (missing.length) {
  console.error(`\nmissing binaries for: ${missing.join(", ")}`);
  console.error("build them (cargo build --release / cargo zigbuild) or drop them in npm/prebuilt/<key>/");
  process.exit(1);
}
console.log("\nall platform packages staged — pack with: npm pack --workspace=<pkg> (see npm/README.md)");
