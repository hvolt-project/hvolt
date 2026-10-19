// ArkTS module types must resolve: a `@kit.*` import re-exports `@ohos.*` modules, and
// a dotted path mapping is required for the inner ones. When it is missing, moving the
// cursor onto `window.WindowStage` answers "/*unresolved*/ any" (measured on the real
// SDK), which is worse than no hover at all.
import assert from "node:assert/strict";
import test from "node:test";
import { spawn } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const sdk = process.env.HVOLT_SDK;
const root = process.env.HVOLT_TEST_ROOT ?? fileURLToPath(new URL("../../examples/hello-world", import.meta.url));
const server = fileURLToPath(new URL("../dist/server.cjs", import.meta.url));
const file = join(root, "src/EntryAbility.ets");

const skip = !sdk && "HVOLT_SDK not set";

function hover(file, needle) {
  return new Promise((resolve) => {
    const text = readFileSync(file, "utf8");
    const lines = text.split("\n");
    const line = lines.findIndex((l) => l.includes(needle));
    assert.ok(line >= 0, `${needle} not found in ${file}`);
    const character = lines[line].lastIndexOf(needle.split(".").pop());

    const child = spawn(process.execPath, [server, "--sdk", sdk, "--root", root], {
      stdio: ["pipe", "pipe", "inherit"],
    });
    let buf = Buffer.alloc(0);
    const send = (m) =>
      child.stdin.write(`Content-Length: ${Buffer.byteLength(JSON.stringify(m))}\r\n\r\n${JSON.stringify(m)}`);
    child.stdout.on("data", (chunk) => {
      buf = Buffer.concat([buf, chunk]);
      const s = buf.toString();
      const header = /Content-Length: (\d+)\r\n\r\n/.exec(s);
      if (!header) return;
      const start = header.index + header[0].length;
      if (Buffer.byteLength(s.slice(start)) < Number(header[1])) return;
      const message = JSON.parse(s.slice(start, start + Number(header[1])));
      buf = Buffer.alloc(0);
      if (message.id === 1) {
        send({ jsonrpc: "2.0", method: "initialized", params: {} });
        send({
          jsonrpc: "2.0",
          method: "textDocument/didOpen",
          params: { textDocument: { uri: `file://${file}`, languageId: "ets", version: 1, text } },
        });
        setTimeout(
          () =>
            send({
              jsonrpc: "2.0",
              id: 2,
              method: "textDocument/hover",
              params: { textDocument: { uri: `file://${file}` }, position: { line, character } },
            }),
          400,
        );
      }
      if (message.id === 2) {
        child.kill();
        resolve(message.result?.contents?.value ?? "");
      }
    });
    send({ jsonrpc: "2.0", id: 1, method: "initialize", params: { processId: process.pid, rootUri: `file://${root}`, capabilities: {} } });
  });
}

test("a type from a @kit re-exported @ohos module resolves", { skip }, async () => {
  assert.ok(existsSync(file), `missing ${file}`);
  const text = await hover(file, "window.WindowStage");
  assert.doesNotMatch(text, /unresolved/, `hover answered: ${text.slice(0, 120)}`);
  assert.match(text, /WindowStage/);
});
