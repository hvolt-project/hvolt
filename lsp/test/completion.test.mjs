// Completion in the places the editor actually uses: component attributes, declared
// object arguments, and enum members. Each case here was a reported bug.
import assert from "node:assert/strict";
import test from "node:test";
import { spawn } from "node:child_process";
import { readFileSync, writeFileSync, mkdtempSync, mkdirSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const sdk = process.env.HVOLT_SDK;
const skip = !sdk && "HVOLT_SDK not set";
const server = fileURLToPath(new URL("../dist/server.cjs", import.meta.url));
const fixture = fileURLToPath(new URL("./fixtures/mini", import.meta.url));

/** Complete at the `|` marker of one of `lines`, inside a temporary project. */
function completeAt(lines, marker = "|") {
  const root = mkdtempSync(join(tmpdir(), "hvolt-comp-"));
  mkdirSync(join(root, "src/pages"), { recursive: true });
  const at = lines.findIndex((line) => line.includes(marker));
  assert.ok(at >= 0, "the marker is missing");
  const character = lines[at].indexOf(marker);
  const body = lines.map((line) => line.replace(marker, ""));

  const source = [
    "@Entry",
    "@Component",
    "struct Index {",
    "  @State message: string = 'Hello';",
    "",
    "  build() {",
    "    Column() {",
    ...body,
    "    }",
    "  }",
    "}",
    "",
  ].join("\n");
  const file = join(root, "src/pages/Index.ets");
  writeFileSync(file, source);

  for (const relative of ["hvolt.toml", "src/hvolt.toml", "src/EntryAbility.ets"]) {
    writeFileSync(join(root, relative), readFileSync(join(fixture, relative), "utf8"));
  }
  // Body lines start after `build()` and `Column() {` — 7 lines of scaffolding.
  const position = { line: 7 + at, character };

  return new Promise((resolve, reject) => {
    const uri = `file://${file}`;
    const child = spawn(process.execPath, ["--max-old-space-size=4096", server, "--sdk", sdk, "--root", root], {
      stdio: ["pipe", "pipe", "inherit"],
    });
    let buf = Buffer.alloc(0);
    const send = (m) => child.stdin.write(`Content-Length: ${Buffer.byteLength(JSON.stringify(m))}\r\n\r\n${JSON.stringify(m)}`);
    const timer = setTimeout(() => {
      child.kill();
      reject(new Error("no completion response"));
    }, 20000);
    child.stdout.on("data", (chunk) => {
      buf = Buffer.concat([buf, chunk]);
      for (;;) {
        const header = /Content-Length: (\d+)\r\n\r\n/.exec(buf.toString("utf8"));
        if (!header) break;
        const headerBytes = Buffer.byteLength(buf.toString("utf8").slice(0, header.index + header[0].length));
        const length = Number(header[1]);
        if (buf.length - headerBytes < length) break;
        const payload = buf.subarray(headerBytes, headerBytes + length).toString("utf8");
        buf = buf.subarray(headerBytes + length);
        const message = JSON.parse(payload);
        if (message.id === 1) {
          send({ jsonrpc: "2.0", method: "initialized", params: {} });
          send({ jsonrpc: "2.0", method: "textDocument/didOpen", params: { textDocument: { uri, languageId: "ets", version: 1, text: source } } });
          setTimeout(
            () => send({ jsonrpc: "2.0", id: 2, method: "textDocument/completion", params: { textDocument: { uri }, position } }),
            600,
          );
        }
        if (message.id === 2) {
          clearTimeout(timer);
          child.kill();
          const items = Array.isArray(message.result) ? message.result : message.result?.items ?? [];
          resolve(items.map((item) => item.label));
        }
      }
    });
    send({ jsonrpc: "2.0", id: 1, method: "initialize", params: { processId: process.pid, rootUri: `file://${root}`, capabilities: {} } });
  });
}

test("a component attribute chain lists attributes and filters by prefix", { skip }, async () => {
  const labels = await completeAt(["      Text(this.message)", "        .align|"]);
  assert.ok(labels.includes("align"), `align missing from ${labels.slice(0, 8)}`);
  assert.ok(labels.includes("alignSelf"), "an inherited attribute is missing");
  assert.ok(!labels.includes("Text"), "components leaked into the attribute list");
});

test("an empty member access lists many attributes", { skip }, async () => {
  const labels = await completeAt(["      Text(this.message)", "        .|"]);
  assert.ok(labels.length > 100, `only ${labels.length} attributes`);
});

test("inside the call, enum members are offered instead of attributes", { skip }, async () => {
  // The reported bug: `.align(Alignment.` offered accessibilityChecked, accessibilityGroup, …
  const labels = await completeAt(["      Text(this.message)", "        .align(Alignment.|"]);
  assert.ok(labels.includes("Center"), `enum members missing: ${labels.slice(0, 8)}`);
  assert.ok(!labels.includes("accessibilityChecked"), "component attributes leaked into an argument");
});

test("an enum member list is complete enough to be useful", { skip }, async () => {
  const labels = await completeAt(["      Text(this.message)", "        .fontWeight(FontWeight.|"]);
  assert.ok(labels.includes("Bold"));
  assert.ok(labels.includes("Regular"));
});

test("a declared object argument lists its properties", { skip }, async () => {
  const labels = await completeAt(["      Text(this.message)", "        .alignRules({|"]);
  for (const key of ["center", "top", "bottom"]) {
    assert.ok(labels.includes(key), `${key} missing from ${labels.slice(0, 8)}`);
  }
});
