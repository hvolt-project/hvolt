// Navigation features that editors expect from a best-in-class language plugin:
// find-references, outline (document symbols) and quick fixes.
import assert from "node:assert/strict";
import test from "node:test";
import { spawn } from "node:child_process";
import { mkdtempSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const sdk = process.env.HVOLT_SDK;
const skip = !sdk && "HVOLT_SDK not set";
const server = fileURLToPath(new URL("../dist/server.cjs", import.meta.url));
const fixture = fileURLToPath(new URL("./fixtures/mini", import.meta.url));

const SOURCE = [
  "@Entry",
  "@Component",
  "struct Index {",
  "  @State message: string = 'Hello';",
  "",
  "  build() {",
  "    Column() {",
  "      Text(this.message).fontSize(20)",
  "    }",
  "  }",
  "}",
].join("\n");

/** Issue several requests against one server and return their results by id. */
function requests(list) {
  const root = mkdtempSync(join(tmpdir(), "hvolt-nav-"));
  mkdirSync(join(root, "src/pages"), { recursive: true });
  for (const relative of ["hvolt.toml", "src/hvolt.toml", "src/EntryAbility.ets"]) {
    writeFileSync(join(root, relative), readFileSync(join(fixture, relative), "utf8"));
  }
  const file = join(root, "src/pages/Index.ets");
  writeFileSync(file, SOURCE);
  const uri = `file://${file}`;

  return new Promise((resolve, reject) => {
    const child = spawn(process.execPath, ["--max-old-space-size=4096", server, "--sdk", sdk, "--root", root], {
      stdio: ["pipe", "pipe", "inherit"],
    });
    let buf = Buffer.alloc(0);
    const answers = new Map();
    const send = (m) => child.stdin.write(`Content-Length: ${Buffer.byteLength(JSON.stringify(m))}\r\n\r\n${JSON.stringify(m)}`);
    const timer = setTimeout(() => {
      child.kill();
      reject(new Error(`missing answers: ${list.map((item) => item.id).filter((id) => !answers.has(id))}`));
    }, 25000);
    child.stdout.on("data", (chunk) => {
      buf = Buffer.concat([buf, chunk]);
      for (;;) {
        const text = buf.toString("utf8");
        const header = /Content-Length: (\d+)\r\n\r\n/.exec(text);
        if (!header) break;
        const hb = Buffer.byteLength(text.slice(0, header.index + header[0].length));
        const length = Number(header[1]);
        if (buf.length - hb < length) break;
        const payload = buf.subarray(hb, hb + length).toString("utf8");
        buf = buf.subarray(hb + length);
        const message = JSON.parse(payload);
        if (message.id === 1) {
          send({ jsonrpc: "2.0", method: "initialized", params: {} });
          send({ jsonrpc: "2.0", method: "textDocument/didOpen", params: { textDocument: { uri, languageId: "ets", version: 1, text: SOURCE } } });
          setTimeout(() => {
            for (const item of list) send({ jsonrpc: "2.0", id: item.id, method: item.method, params: item.params(uri) });
          }, 700);
        } else if (message.id !== undefined && message.id !== 1) {
          answers.set(message.id, message.result ?? null);
          if (list.every((item) => answers.has(item.id))) {
            clearTimeout(timer);
            child.kill();
            resolve(answers);
          }
        }
      }
    });
    send({ jsonrpc: "2.0", id: 1, method: "initialize", params: { processId: process.pid, rootUri: `file://${root}`, capabilities: {} } });
  });
}

test("find-references covers the declaration and its uses", { skip }, async () => {
  const answers = await requests([
    {
      id: 2,
      method: "textDocument/references",
      params: (uri) => ({ textDocument: { uri }, position: { line: 3, character: 12 }, context: { includeDeclaration: true } }),
    },
  ]);
  const locations = answers.get(2) ?? [];
  assert.ok(locations.length >= 2, `only ${locations.length} reference(s)`);
  assert.ok(locations.every((location) => location.uri.startsWith("file://")));
});

test("the outline shows the struct and its members", { skip }, async () => {
  const answers = await requests([{ id: 2, method: "textDocument/documentSymbol", params: (uri) => ({ textDocument: { uri } }) }]);
  const symbols = answers.get(2) ?? [];
  const index = symbols.find((symbol) => symbol.name === "Index");
  assert.ok(index, `no Index in ${symbols.map((symbol) => symbol.name)}`);
  const names = (index.children ?? []).map((child) => child.name);
  assert.ok(names.includes("build"), `build missing: ${names}`);
  assert.ok(names.includes("message"), `message missing: ${names}`);
});

test("a quick fix is offered for a type error", { skip }, async () => {
  const answers = await requests([
    {
      id: 2,
      method: "textDocument/codeAction",
      params: (uri) => ({
        textDocument: { uri },
        range: { start: { line: 7, character: 6 }, end: { line: 7, character: 24 } },
        context: {
          diagnostics: [
            { range: { start: { line: 7, character: 6 }, end: { line: 7, character: 24 } }, message: "cannot find", code: 2339 },
          ],
        },
      }),
    },
  ]);
  const actions = answers.get(2) ?? [];
  assert.ok(actions.length >= 1, "no code action for a known error code");
  assert.ok(actions[0].edit || actions[0].command, "an action without an edit");
});
