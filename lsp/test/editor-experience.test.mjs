// The four features an editor user feels most: occurrences highlighted in the file,
// inlay hints, and a completion that carries documentation and its own import.
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
  "    console.log(this.message)",
  "  }",
  "}",
].join("\n");

/** A project with one exporting module, plus a helper that issues requests by id. */
function session() {
  const root = mkdtempSync(join(tmpdir(), "hvolt-exp-"));
  mkdirSync(join(root, "src/pages"), { recursive: true });
  mkdirSync(join(root, "src/model"), { recursive: true });
  for (const relative of ["hvolt.toml", "src/hvolt.toml", "src/EntryAbility.ets"]) {
    writeFileSync(join(root, relative), readFileSync(join(fixture, relative), "utf8"));
  }
  writeFileSync(
    join(root, "src/model/Store.ets"),
    "/** Adds one to the number it is given. */\nexport function addOne(value: number): number {\n  return value + 1;\n}\n",
  );
  const file = join(root, "src/pages/Index.ets");
  writeFileSync(file, SOURCE);
  const uri = `file://${file}`;
  const lines = SOURCE.split("\n");
  const at = (needle) => {
    const line = lines.findIndex((text) => text.includes(needle));
    return { line, character: lines[line].indexOf(needle) + 1 };
  };

  return new Promise((resolve, reject) => {
    const child = spawn(process.execPath, ["--max-old-space-size=4096", server, "--sdk", sdk, "--root", root], {
      stdio: ["pipe", "pipe", "inherit"],
    });
    let buf = Buffer.alloc(0);
    const send = (m) => child.stdin.write(`Content-Length: ${Buffer.byteLength(JSON.stringify(m))}\r\n\r\n${JSON.stringify(m)}`);
    const answers = new Map();
    const wanted = new Set([2, 3, 4, 5]);
    const timer = setTimeout(() => {
      child.kill();
      reject(new Error(`missing: ${[...wanted].filter((id) => !answers.has(id))}`));
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
            const logCall = at("console.log(");
            send({ jsonrpc: "2.0", id: 2, method: "textDocument/documentHighlight", params: { textDocument: { uri }, position: at("message") } });
            send({
              jsonrpc: "2.0",
              id: 3,
              method: "textDocument/inlayHint",
              params: { textDocument: { uri }, range: { start: { line: 0, character: 0 }, end: { line: lines.length - 1, character: 1 } } },
            });
            send({
              jsonrpc: "2.0",
              id: 4,
              method: "textDocument/completion",
              params: { textDocument: { uri }, position: { line: logCall.line, character: logCall.character + "console.log(".length } },
            });
          }, 900);
        } else if (message.id === 4) {
          const items = message.result ?? [];
          answers.set(4, items);
          const exported = items.find((item) => item.label === "addOne");
          send({ jsonrpc: "2.0", id: 5, method: "completionItem/resolve", params: exported ?? items[0] });
        } else if (message.id !== undefined && message.id !== 1) {
          answers.set(message.id, message.result ?? null);
        }
        if ([...wanted].every((id) => answers.has(id))) {
          clearTimeout(timer);
          child.kill();
          resolve(answers);
        }
      }
    });
    send({ jsonrpc: "2.0", id: 1, method: "initialize", params: { processId: process.pid, rootUri: `file://${root}`, capabilities: {} } });
  });
}

test("occurrences of a symbol are highlighted in the file", { skip }, async () => {
  const highlights = (await session()).get(2) ?? [];
  assert.ok(highlights.length >= 2, `only ${highlights.length} occurrence(s)`);
  assert.ok(highlights.every((highlight) => typeof highlight.range.start.line === "number"));
});

test("inlay hints name parameters and return types", { skip }, async () => {
  const hints = (await session()).get(3) ?? [];
  assert.ok(hints.length >= 1, "no inlay hints");
  const labels = hints.map((hint) => hint.label);
  assert.ok(labels.some((label) => /^[.a-z]+:$/.test(label) || /^: /.test(label)), `labels were ${labels}`);
});

test("a project export is offered, documented and imported on accept", { skip }, async () => {
  const answers = await session();
  const items = answers.get(4) ?? [];
  assert.ok(items.some((item) => item.label === "addOne"), `addOne missing from ${items.length} items`);
  const resolved = answers.get(5) ?? {};
  assert.equal(resolved.label, "addOne");
  assert.match(resolved.documentation?.value ?? "", /Adds one/);
  const edits = resolved.additionalTextEdits ?? [];
  assert.equal(edits.length, 1, JSON.stringify(edits).slice(0, 160));
  assert.match(edits[0].newText, /import \{ addOne \} from "\.\/\.\.\/model\/Store"|from "\.\.\/model\/Store"/);
});
