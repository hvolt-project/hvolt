// Capability matrix — what the server actually answers at the places an editor user runs
// into. Each row is a measured claim, so a gap cannot hide behind "it works for the case
// I implemented". Rows marked todo are measured gaps with the reason, not passes.
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

/** Run one probe at the `|` marker of `body` and return the raw answer. */
function ask(probe, body) {
  const root = mkdtempSync(join(tmpdir(), "hvolt-matrix-"));
  mkdirSync(join(root, "src/pages"), { recursive: true });
  for (const relative of ["hvolt.toml", "src/hvolt.toml", "src/EntryAbility.ets"]) {
    writeFileSync(join(root, relative), readFileSync(join(fixture, relative), "utf8"));
  }
  const at = body.findIndex((line) => line.includes("|"));
  const character = body[at].indexOf("|");
  const lines = body.map((line) => line.replace("|", ""));
  const source = [
    "import { hilog } from '@kit.PerformanceAnalysisKit';",
    "@Entry",
    "@Component",
    "struct Index {",
    "  @State message: string = 'Hello';",
    "",
    "  build() {",
    "    Column() {",
    ...lines,
    "    }",
    "  }",
    "}",
    "",
  ].join("\n");
  const file = join(root, "src/pages/Matrix.ets");
  writeFileSync(file, source);
  const uri = `file://${file}`;
  const position = { line: 8 + at, character };
  const method =
    probe === "hover" ? "textDocument/hover" : probe === "completion" ? "textDocument/completion" : "textDocument/signatureHelp";

  return new Promise((resolve, reject) => {
    const child = spawn(process.execPath, ["--max-old-space-size=4096", server, "--sdk", sdk, "--root", root], {
      stdio: ["pipe", "pipe", "inherit"],
    });
    let buf = Buffer.alloc(0);
    const send = (m) => child.stdin.write(`Content-Length: ${Buffer.byteLength(JSON.stringify(m))}\r\n\r\n${JSON.stringify(m)}`);
    const timer = setTimeout(() => {
      child.kill();
      reject(new Error(`${probe} at ${position.line}:${position.character} timed out`));
    }, 20000);
    child.stdout.on("data", (chunk) => {
      buf = Buffer.concat([buf, chunk]);
      for (;;) {
        const header = /Content-Length: (\d+)\r\n\r\n/.exec(buf.toString("utf8"));
        if (!header) break;
        const hb = Buffer.byteLength(buf.toString("utf8").slice(0, header.index + header[0].length));
        const length = Number(header[1]);
        if (buf.length - hb < length) break;
        const payload = buf.subarray(hb, hb + length).toString("utf8");
        buf = buf.subarray(hb + length);
        const message = JSON.parse(payload);
        if (message.id === 1) {
          send({ jsonrpc: "2.0", method: "initialized", params: {} });
          send({ jsonrpc: "2.0", method: "textDocument/didOpen", params: { textDocument: { uri, languageId: "ets", version: 1, text: source } } });
          setTimeout(() => send({ jsonrpc: "2.0", id: 2, method, params: { textDocument: { uri }, position } }), 500);
        }
        if (message.id === 2) {
          clearTimeout(timer);
          child.kill();
          resolve(message.result ?? null);
        }
      }
    });
    send({ jsonrpc: "2.0", id: 1, method: "initialize", params: { processId: process.pid, rootUri: `file://${root}`, capabilities: {} } });
  });
}

const hoverText = (answer) => answer?.contents?.value ?? "";
const labels = (answer) => {
  const items = Array.isArray(answer) ? answer : answer?.items ?? [];
  return items.map((item) => item.label);
};
const signature = (answer) => answer?.signatures?.[0]?.label ?? "";

test("hover: local variable, parameter, enum member, component, attribute", { skip }, async () => {
  assert.match(hoverText(await ask("hover", ["    let counter: number = 1;", "    counter|;"])), /counter: number/);
  assert.match(hoverText(await ask("hover", ["    [1].forEach((item|) => {})"])), /item: number/);
  assert.match(hoverText(await ask("hover", ["    Text(this.message).fontWeight(FontWeight.Bo|ld)"])), /FontWeight\.Bold/);
  assert.match(hoverText(await ask("hover", ["    Te|xt(this.message)"])), /Text/);
  assert.match(hoverText(await ask("hover", ["    Text(this.message).fontWe|ight(1)"])), /fontWeight\(/);
});

test("completion: module members and enum names inside a call", { skip }, async () => {
  const members = labels(await ask("completion", ["    hilog.|"]));
  assert.ok(members.includes("info") && members.includes("error"), `hilog members: ${members.slice(0, 6)}`);
  const enumNames = labels(await ask("completion", ["    Text(this.message).fontWeight(|)"]));
  assert.equal(enumNames[0], "FontWeight");
});

test("signature help: plain call and ArkUI attribute", { skip }, async () => {
  assert.match(signature(await ask("signature", ["    console.log(|)"])), /log\(/);
  assert.match(signature(await ask("signature", ["    Text(this.message).fontWeight(|)"])), /fontWeight\(/);
});

// Measured gaps. Inside `build()` the SDK's parser rejects the ArkUI block, so the
// language service has no scope at all: even `coun` returns the 1,757 global entries.
// Every item below needs an ArkTS→TS transform, not another special case.
test("completion: a local declared in the same build() body", { skip, todo: "needs an ArkTS transform; the fork has no scope inside build()" }, async () => {
  const found = labels(await ask("completion", ["    let counter: number = 1;", "    coun|"]));
  assert.ok(found.includes("counter"), `only globals came back: ${found.slice(0, 6)}`);
});

test("completion: an imported name", { skip, todo: "same cause: imports are invisible inside build()" }, async () => {
  const found = labels(await ask("completion", ["    hil|"]));
  assert.deepEqual(found, ["hilog"]);
});

test("completion: a keyword", { skip, todo: "keywords are merged, but ranked below the SDK's globals" }, async () => {
  const found = labels(await ask("completion", ["    con|"]));
  assert.equal(found[0], "const");
});
