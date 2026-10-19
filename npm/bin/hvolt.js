#!/usr/bin/env node
// Launcher: resolves the platform-specific package installed as an optional
// dependency and execs the prebuilt binary with this process's stdio.
"use strict";

const { spawn } = require("node:child_process");
const path = require("node:path");

const SUPPORTED = ["linux-x64", "linux-arm64", "darwin-x64", "darwin-arm64", "win32-x64"];

function platformKey() {
  const key = `${process.platform}-${process.arch}`;
  if (!SUPPORTED.includes(key)) {
    console.error(`hvolt: no prebuilt binary for ${key} (supported: ${SUPPORTED.join(", ")})`);
    console.error("hvolt: build from source instead — https://github.com/hvolt-project/hvolt");
    process.exit(1);
  }
  return key;
}

function binaryPath(key) {
  const pkg = `@hvolt/cli-${key}`;
  const name = process.platform === "win32" ? "hvolt.exe" : "hvolt";
  try {
    return require.resolve(`${pkg}/bin/${name}`);
  } catch {
    console.error(`hvolt: the optional dependency ${pkg} is not installed.`);
    console.error("hvolt: reinstall without --no-optional, or install it explicitly:");
    console.error(`  npm install ${pkg}`);
    process.exit(1);
  }
}

const key = platformKey();
// The language server bundle ships in this (platform-independent) package; the
// binary looks it up through HVOLT_LSP_SERVER.
if (!process.env.HVOLT_LSP_SERVER) {
  const bundled = path.join(__dirname, "..", "lsp", "dist", "server.cjs");
  if (require("node:fs").existsSync(bundled)) process.env.HVOLT_LSP_SERVER = bundled;
}
const child = spawn(binaryPath(key), process.argv.slice(2), { stdio: "inherit" });

for (const sig of ["SIGINT", "SIGTERM", "SIGHUP"]) {
  process.on(sig, () => {
    if (!child.killed) child.kill(sig);
  });
}

child.on("error", (err) => {
  console.error(`hvolt: failed to start the binary: ${err.message}`);
  process.exit(1);
});
child.on("exit", (code, signal) => {
  if (signal) {
    process.kill(process.pid, signal);
    return;
  }
  process.exit(code === null ? 1 : code);
});
