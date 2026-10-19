// The protocol log must record what the editor actually sent: initialize and the
// documents it opened. Needs an SDK (the server resolves it at startup); skips
// without HVOLT_SDK, like the other integration checks.
import assert from "node:assert/strict";
import test from "node:test";
import { spawn } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const sdk = process.env.HVOLT_SDK;
const server = fileURLToPath(new URL("../dist/server.cjs", import.meta.url));

test("initialize and didOpen are recorded", { skip: !sdk && "HVOLT_SDK not set" }, async () => {
  const dir = mkdtempSync(join(tmpdir(), "hvolt-log-"));
  const log = join(dir, "lsp.log");
  const file = join(dir, "Sample.ets");
  const text = "struct A {\n  build() {}\n}\n";
  writeFileSync(file, text);

  const child = spawn(process.execPath, [server, "--sdk", sdk, "--root", dir, "--log-file", log], {
    stdio: ["pipe", "pipe", "inherit"],
  });
  let buf = Buffer.alloc(0);
  const send = (message) =>
    child.stdin.write(`Content-Length: ${Buffer.byteLength(JSON.stringify(message))}\r\n\r\n${JSON.stringify(message)}`);
  const request = (id, method, params) => send({ jsonrpc: "2.0", id, method, params });

  const done = new Promise((resolve) => {
    child.stdout.on("data", (chunk) => {
      buf = Buffer.concat([buf, chunk]);
      const body = buf.toString().match(/\{[\s\S]*\}$/);
      if (body) resolve(body[0]);
    });
  });

  request(1, "initialize", { processId: process.pid, rootUri: `file://${dir}`, capabilities: {} });
  await done;
  send({ jsonrpc: "2.0", method: "initialized", params: {} });
  send({
    jsonrpc: "2.0",
    method: "textDocument/didOpen",
    params: { textDocument: { uri: `file://${file}`, languageId: "ets", version: 1, text } },
  });
  await new Promise((r) => setTimeout(r, 700));
  child.kill();

  assert.ok(existsSync(log), "no log file was written");
  const lines = readFileSync(log, "utf8").trim().split("\n");
  assert.ok(lines.some((l) => l.includes("initialize rootUri=")), `missing initialize: ${lines.join(" | ")}`);
  assert.ok(
    lines.some((l) => l.includes("didOpen") && l.includes("Sample.ets") && l.includes(`ets, ${text.split("\n").length} lines`)),
    `missing didOpen: ${lines.join(" | ")}`,
  );
});
