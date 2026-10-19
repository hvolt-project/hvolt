// The editor command → CLI argument mapping.
import assert from "node:assert/strict";
import test from "node:test";
import { COMMANDS, commandSpec } from "../dist/commands.mjs";

test("build and assemble map straight through", () => {
  assert.deepEqual(commandSpec("hvolt.build")?.args, ["build"]);
  assert.deepEqual(commandSpec("hvolt.assemble")?.args, ["assemble"]);
});

test("sign defaults to the local chain", () => {
  assert.deepEqual(commandSpec("hvolt.sign")?.args, ["sign", "--local"]);
  assert.deepEqual(commandSpec("hvolt.sign", { local: false })?.args, ["sign"]);
});

test("device is forwarded where it matters", () => {
  assert.deepEqual(commandSpec("hvolt.push", { device: "192.168.1.7:38565" })?.args, [
    "push",
    "--device",
    "192.168.1.7:38565",
  ]);
  assert.deepEqual(commandSpec("hvolt.push")?.args, ["push"]);
  assert.deepEqual(commandSpec("hvolt.screenshot", { out: "/tmp/s.jpeg" })?.args, [
    "screenshot",
    "--out",
    "/tmp/s.jpeg",
  ]);
});

test("log is marked long-running, others are not", () => {
  assert.equal(commandSpec("hvolt.log")?.longRunning, true);
  assert.equal(commandSpec("hvolt.build")?.longRunning, undefined);
});

test("unknown commands are rejected and the list is complete", () => {
  assert.equal(commandSpec("hvolt.nope"), null);
  assert.ok(COMMANDS.includes("hvolt.run"));
  assert.ok(!COMMANDS.includes("hvolt.buildDiagnostics"), "diagnostics is separate from the CLI map");
});
