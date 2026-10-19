// ArkTS's own DSL surface: decorators (`@Component`, `@State`, …) and the resource
// functions (`$r`, `$rawfile`). Both are plain globals to the type checker, so they are
// buried in the shared list or missing entirely inside a component chain.
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

function ask(probe, lines) {
  const root = mkdtempSync(join(tmpdir(), "hvolt-dsl-"));
  mkdirSync(join(root, "src/pages"), { recursive: true });
  for (const relative of ["hvolt.toml", "src/hvolt.toml", "src/EntryAbility.ets"]) {
    writeFileSync(join(root, relative), readFileSync(join(fixture, relative), "utf8"));
  }
  const at = lines.findIndex((line) => line.includes("|"));
  const character = lines[at].indexOf("|");
  const before = lines[at].slice(0, character);
  const prefix = (/[@A-Za-z_$][\w$]*$/.exec(before) ?? [""])[0].replace(/^@/, "");
  const source = lines.map((line) => line.replace("|", "")).join("\n");
  const file = join(root, "src/pages/D.ets");
  writeFileSync(file, source);
  const uri = `file://${file}`;
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
      reject(new Error("no response"));
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
          send({ jsonrpc: "2.0", method: "textDocument/didOpen", params: { textDocument: { uri, languageId: "ets", version: 1, text: source } } });
          setTimeout(() => send({ jsonrpc: "2.0", id: 2, method, params: { textDocument: { uri }, position: { line: at, character } } }), 600);
        }
        if (message.id === 2) {
          clearTimeout(timer);
          child.kill();
          resolve({ result: message.result ?? null, prefix });
        }
      }
    });
    send({ jsonrpc: "2.0", id: 1, method: "initialize", params: { processId: process.pid, rootUri: `file://${root}`, capabilities: {} } });
  });
}

const labels = ({ result, prefix }) => {
  const items = Array.isArray(result) ? result : result?.items ?? [];
  const names = items.map((item) => item.label);
  return prefix ? names.filter((name) => name.startsWith(prefix)) : names;
};

test("@ offers the ArkTS decorators", { skip }, async () => {
  const found = labels(await ask("completion", ["@|", "struct C {", "  build() { Column() {} }", "}"]));
  for (const name of ["Component", "Entry", "State", "Prop", "Link", "Builder"]) {
    assert.ok(found.includes(name), `${name} missing: ${found.slice(0, 8)}`);
  }
});

test("a decorator prefix filters to the matching decorators", { skip }, async () => {
  const found = labels(await ask("completion", ["struct C {", "  @Sta|", "  message: string = 'a';", "  build() { Column() {} }", "}"]));
  assert.equal(found[0], "State", `first was ${found[0]}`);
});

test("hover explains a decorator's declared type", { skip }, async () => {
  const { result } = await ask("hover", ["@Comp|onent", "struct C {", "  build() { Column() {} }", "}"]);
  assert.match(result?.contents?.value ?? "", /ClassDecorator/);
});

test("$r completion and signature work inside a chain", { skip }, async () => {
  const found = labels(await ask("completion", ["struct C {", "  build() { Text($r|()) }", "}"]));
  assert.ok(found.includes("$r"), `$r missing: ${found.slice(0, 8)}`);
  const { result } = await ask("signature", ["struct C {", "  build() { Text($r(|)) }", "}"]);
  assert.match(result?.signatures?.[0]?.label ?? "", /\$r\(/);
});
