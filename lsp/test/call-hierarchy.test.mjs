// Call hierarchy: who calls this, and what this calls.
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

const MATH = [
  "export function add(a: number, b: number): number {",
  "  return a + b;",
  "}",
  "",
  "export function twice(a: number): number {",
  "  return add(a, a);",
  "}",
].join("\n");

const PAGE = [
  "import { twice, add } from '../util/Math';",
  "",
  "@Entry",
  "@Component",
  "struct Index {",
  "  total(): number {",
  "    return twice(2) + add(1, 2);",
  "  }",
  "",
  "  build() {",
  "    Column() {",
  "      Text(String(this.total())).fontSize(20)",
  "    }",
  "  }",
  "}",
].join("\n");

function project() {
  const root = mkdtempSync(join(tmpdir(), "hvolt-call-"));
  mkdirSync(join(root, "src/pages"), { recursive: true });
  mkdirSync(join(root, "src/util"), { recursive: true });
  for (const relative of ["hvolt.toml", "src/hvolt.toml", "src/EntryAbility.ets"]) {
    writeFileSync(join(root, relative), readFileSync(join(fixture, relative), "utf8"));
  }
  writeFileSync(join(root, "src/util/Math.ets"), MATH);
  const page = join(root, "src/pages/Index.ets");
  writeFileSync(page, PAGE);
  return { root, page, math: join(root, "src/util/Math.ets") };
}

const lineOf = (text, needle) => text.split("\n").findIndex((line) => line.includes(needle));
const columnOf = (text, needle, extra = 1) => text.split("\n")[lineOf(text, needle)].indexOf(needle) + extra;

/** Ask a list of requests, threading earlier answers into later parameters. */
function ask(root, page, math, steps) {
  return new Promise((resolve, reject) => {
    const child = spawn(process.execPath, ["--max-old-space-size=4096", server, "--sdk", sdk, "--root", root], {
      stdio: ["pipe", "pipe", "inherit"],
    });
    let buf = Buffer.alloc(0);
    const send = (m) => child.stdin.write(`Content-Length: ${Buffer.byteLength(JSON.stringify(m))}\r\n\r\n${JSON.stringify(m)}`);
    const answers = new Map();
    const timer = setTimeout(() => {
      child.kill();
      reject(new Error(`missing: ${[...Array(steps.length).keys()].map((index) => index + 2).filter((id) => !answers.has(id))}`));
    }, 40000);
    const run = (index) => {
      if (index >= steps.length) {
        clearTimeout(timer);
        child.kill();
        resolve(answers);
        return;
      }
      const step = steps[index];
      send({ jsonrpc: "2.0", id: index + 2, method: step.method, params: step.params(answers) });
    };
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
          send({ jsonrpc: "2.0", method: "textDocument/didOpen", params: { textDocument: { uri: `file://${page}`, languageId: "ets", version: 1, text: PAGE } } });
          send({ jsonrpc: "2.0", method: "textDocument/didOpen", params: { textDocument: { uri: `file://${math}`, languageId: "ets", version: 1, text: MATH } } });
          setTimeout(() => run(0), 1000);
        } else if (message.id >= 2) {
          answers.set(message.id, message.result ?? null);
          run(message.id - 1);
        }
      }
    });
    send({ jsonrpc: "2.0", id: 1, method: "initialize", params: { processId: process.pid, rootUri: `file://${root}`, capabilities: {} } });
  });
}

const prepare = (text, needle) => (answers) => {
  void answers;
  return { textDocument: { uri: `file://${text.uri}` }, position: { line: lineOf(text.body, needle), character: columnOf(text.body, needle) } };
};

test("prepare names the function under the cursor", { skip }, async () => {
  const { root, page, math } = project();
  const answers = await ask(root, page, math, [
    { method: "textDocument/prepareCallHierarchy", params: prepare({ uri: math, body: MATH }, "add(a, a)") },
  ]);
  const items = answers.get(2) ?? [];
  assert.equal(items.length, 1, JSON.stringify(items).slice(0, 160));
  assert.equal(items[0].name, "add");
  assert.ok(items[0].uri.endsWith("Math.ets"));
});

test("incoming calls list the caller", { skip }, async () => {
  const { root, page, math } = project();
  const answers = await ask(root, page, math, [
    { method: "textDocument/prepareCallHierarchy", params: prepare({ uri: math, body: MATH }, "add(a, a)") },
    { method: "callHierarchy/incomingCalls", params: (answers) => ({ item: (answers.get(2) ?? [])[0] }) },
  ]);
  const calls = answers.get(3) ?? [];
  assert.ok(calls.some((call) => call.from.name === "twice"), JSON.stringify(calls.map((call) => call.from.name)));
});

test("outgoing calls list the callee", { skip }, async () => {
  const { root, page, math } = project();
  const answers = await ask(root, page, math, [
    { method: "textDocument/prepareCallHierarchy", params: prepare({ uri: math, body: MATH }, "twice(a: number)") },
    { method: "callHierarchy/outgoingCalls", params: (answers) => ({ item: (answers.get(2) ?? [])[0] }) },
  ]);
  const calls = answers.get(3) ?? [];
  assert.ok(calls.some((call) => call.to.name === "add"), JSON.stringify(calls.map((call) => call.to.name)));
  assert.ok((calls[0].fromRanges ?? []).length >= 1, "an outgoing call should point at the call site");
});
