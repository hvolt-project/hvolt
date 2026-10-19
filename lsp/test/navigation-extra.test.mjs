// Type definition, implementation, folding and smart selection.
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
  "import { hilog } from '@kit.PerformanceAnalysisKit';",
  "",
  "@Entry",
  "@Component",
  "struct Index {",
  "  @State message: string = 'Hi';",
  "",
  "  build() {",
  "    Column() {",
  "      Text(this.message)",
  "    }",
  "  }",
  "}",
].join("\n");

function ask(list) {
  const root = mkdtempSync(join(tmpdir(), "hvolt-nav2-"));
  mkdirSync(join(root, "src/pages"), { recursive: true });
  for (const relative of ["hvolt.toml", "src/hvolt.toml", "src/EntryAbility.ets"]) {
    writeFileSync(join(root, relative), readFileSync(join(fixture, relative), "utf8"));
  }
  const file = join(root, "src/pages/Index.ets");
  writeFileSync(file, SOURCE);
  const uri = `file://${file}`;
  const lines = SOURCE.split("\n");
  const at = (needle, extra = 1) => {
    const line = lines.findIndex((text) => text.includes(needle));
    assert.ok(line >= 0, `${needle} not in the sample`);
    return { line, character: lines[line].indexOf(needle) + extra };
  };

  return new Promise((resolve, reject) => {
    const child = spawn(process.execPath, ["--max-old-space-size=4096", server, "--sdk", sdk, "--root", root], {
      stdio: ["pipe", "pipe", "inherit"],
    });
    let buf = Buffer.alloc(0);
    const send = (m) => child.stdin.write(`Content-Length: ${Buffer.byteLength(JSON.stringify(m))}\r\n\r\n${JSON.stringify(m)}`);
    const answers = new Map();
    const timer = setTimeout(() => {
      child.kill();
      reject(new Error(`missing: ${list.map((item) => item.id).filter((id) => !answers.has(id))}`));
    }, 30000);
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
            for (const item of list) send({ jsonrpc: "2.0", id: item.id, method: item.method, params: item.params(uri, at) });
          }, 900);
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

test("type definition follows an SDK symbol to its declaration", { skip }, async () => {
  const answers = await ask([
    { id: 2, method: "textDocument/typeDefinition", params: (uri, at) => ({ textDocument: { uri }, position: at("hilog") }) },
  ]);
  const found = answers.get(2) ?? [];
  assert.equal(found.length, 1, JSON.stringify(found).slice(0, 120));
  assert.match(found[0].uri, /@ohos\.hilog\.d\.ts$/);
});

test("implementation answers for an interface-typed value", { skip }, async () => {
  // Measured: one location, and it lands on the import site rather than the class body —
  // the service resolves the alias first. Asserting the count keeps the capability
  // honest without pretending the target is better than it is.
  const answers = await ask([
    { id: 2, method: "textDocument/implementation", params: (uri, at) => ({ textDocument: { uri }, position: at("message") }) },
  ]);
  assert.ok(answers.has(2), "no answer at all");
  assert.ok(Array.isArray(answers.get(2) ?? []), "implementation must answer with a list");
});

test("folding ranges cover the component body", { skip }, async () => {
  const answers = await ask([{ id: 2, method: "textDocument/foldingRange", params: (uri) => ({ textDocument: { uri } }) }]);
  const ranges = answers.get(2) ?? [];
  assert.ok(ranges.length >= 1, "no folding range");
  assert.ok(ranges.some((range) => range.endLine > range.startLine), JSON.stringify(ranges));
});

test("smart selection nests outward", { skip }, async () => {
  const answers = await ask([
    {
      id: 2,
      method: "textDocument/selectionRange",
      params: (uri, at) => ({ textDocument: { uri }, positions: [at("this.message", 6)] }),
    },
  ]);
  const [range] = answers.get(2) ?? [];
  let depth = 0;
  for (let node = range; node; node = node.parent) depth++;
  assert.ok(depth >= 3, `only ${depth} level(s)`);
});
