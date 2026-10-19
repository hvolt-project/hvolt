// Type diagnostics: they come from the SDK's TypeScript service, must appear for a real
// type error — including one that only exists in the editor buffer — and must stay
// silent for files whose ArkUI block-chains the fork's parser rejects.
import assert from "node:assert/strict";
import test from "node:test";
import { spawn } from "node:child_process";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const sdk = process.env.HVOLT_SDK;
// Hermetic fixture: the example projects get edited while the extension is being
// used, so tests must not depend on their current content.
const root = process.env.HVOLT_TEST_ROOT ?? fileURLToPath(new URL("./fixtures/mini", import.meta.url));
const server = fileURLToPath(new URL("../dist/server.cjs", import.meta.url));
const skip = !sdk && "HVOLT_SDK not set";

function run(file, needle, replacement) {
  const text = readFileSync(join(root, file), "utf8");
  const edited = replacement ? text.replace(needle, replacement) : text;
  const uri = `file://${join(root, file)}`;
  return new Promise((resolve) => {
    const child = spawn(process.execPath, ["--max-old-space-size=4096", server, "--sdk", sdk, "--root", root], { stdio: ["pipe", "pipe", "inherit"], env: { ...process.env, NODE_OPTIONS: "" } });
    let buf = Buffer.alloc(0);
    let seen = [];
    const send = (m) => child.stdin.write(`Content-Length: ${Buffer.byteLength(JSON.stringify(m))}\r\n\r\n${JSON.stringify(m)}`);
    const finish = () => { child.kill(); resolve(seen); };
    child.stdout.on("data", (chunk) => {
      buf = Buffer.concat([buf, chunk]);
      // Consume every complete frame, then keep the remainder: reading from a stale
      // snapshot re-parses the same header forever.
      for (;;) {
        const header = /Content-Length: (\d+)\r\n\r\n/.exec(buf.toString("utf8"));
        if (!header) break;
        const headerBytes = Buffer.byteLength(buf.toString("utf8").slice(0, header.index + header[0].length));
        const length = Number(header[1]);
        if (buf.length - headerBytes < length) break;
        const body = buf.subarray(headerBytes, headerBytes + length).toString("utf8");
        buf = buf.subarray(headerBytes + length);
        const msg = JSON.parse(body);
        if (msg.id === 1) {
          send({ jsonrpc: "2.0", method: "initialized", params: {} });
          send({ jsonrpc: "2.0", method: "textDocument/didOpen", params: { textDocument: { uri, languageId: "ets", version: 1, text } } });
          if (edited !== text) {
            send({ jsonrpc: "2.0", method: "textDocument/didChange", params: { textDocument: { uri, version: 2 }, contentChanges: [{ text: edited }] } });
          }
          setTimeout(finish, 4000);
        }
        if (msg.method === "textDocument/publishDiagnostics" && msg.params.uri === uri) {
          seen = msg.params.diagnostics.filter((d) => d.source === "arkts");
        }
      }
    });
    send({ jsonrpc: "2.0", id: 1, method: "initialize", params: { processId: process.pid, rootUri: `file://${root}`, capabilities: {} } });
  });
}

test("a clean file yields no type diagnostics", { skip }, async () => {
  const diagnostics = await run("src/EntryAbility.ets");
  assert.deepEqual(diagnostics, []);
});

test("the published range covers the offending token", { skip }, async () => {
  const diagnostics = await run(
    "src/EntryAbility.ets",
    "onCreate(want: Want, launchParam: AbilityConstant.LaunchParam): void {",
    "onCreate(want: Want, launchParam: AbilityConstant.LaunchParam): void {\n    const broken: number = 'text';",
  );
  assert.equal(diagnostics.length, 1);
  const { start, end } = diagnostics[0].range;
  assert.ok(end.line === start.line, "a one-line diagnostic");
  assert.ok(
    end.character - start.character > 1,
    `range is only ${end.character - start.character} character wide — a point range, not a token`,
  );
});

test("a type error in the buffer is reported", { skip }, async () => {
  const diagnostics = await run(
    "src/EntryAbility.ets",
    "onCreate(want: Want, launchParam: AbilityConstant.LaunchParam): void {",
    "onCreate(want: Want, launchParam: AbilityConstant.LaunchParam): void {\n    const broken: number = 'text';",
  );
  assert.ok(diagnostics.length >= 1, "no diagnostic for the type error");
  assert.match(diagnostics[0].message, /not assignable to type 'number'/);
});

test("a file with ArkUI block-chains is left alone", { skip }, async () => {
  const diagnostics = await run("src/pages/Index.ets");
  assert.deepEqual(diagnostics, [], "the fork's parse errors must not reach the editor");
});
