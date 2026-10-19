// Workspace-wide search and clickable paths inside hvolt.toml.
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

function project() {
  const root = mkdtempSync(join(tmpdir(), "hvolt-ws-"));
  mkdirSync(join(root, "src/pages"), { recursive: true });
  mkdirSync(join(root, "src/model"), { recursive: true });
  mkdirSync(join(root, "lib"), { recursive: true });
  for (const relative of ["hvolt.toml", "src/hvolt.toml", "src/EntryAbility.ets"]) {
    writeFileSync(join(root, relative), readFileSync(join(fixture, relative), "utf8"));
  }
  // A dependency path is what a link should point at.
  const app = readFileSync(join(root, "hvolt.toml"), "utf8").replace("[dependencies]", '[dependencies]\npath = "./lib"');
  writeFileSync(join(root, "hvolt.toml"), app);
  writeFileSync(join(root, "lib/hvolt.toml"), '[module]\nname = "lib"\nentry = "Lib.ets"\nmodule_type = "har"\n');
  writeFileSync(join(root, "src/model/Store.ets"), "export class Store {\n  count: number = 0;\n}\n");
  const index = join(root, "src/pages/Index.ets");
  writeFileSync(index, "@Entry\n@Component\nstruct Index {\n  build() {\n    Column() {}\n  }\n}\n");
  return { root, index, app: join(root, "hvolt.toml") };
}

function ask(root, index, list) {
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
          send({
            jsonrpc: "2.0",
            method: "textDocument/didOpen",
            params: { textDocument: { uri: `file://${index}`, languageId: "ets", version: 1, text: readFileSync(index, "utf8") } },
          });
          setTimeout(() => {
            for (const item of list) send({ jsonrpc: "2.0", id: item.id, method: item.method, params: item.params });
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

test("workspace symbol finds a class across the project", { skip }, async () => {
  const { root, index } = project();
  const answers = await ask(root, index, [{ id: 2, method: "workspace/symbol", params: { query: "Sto" } }]);
  const found = answers.get(2) ?? [];
  assert.ok(found.some((symbol) => symbol.name === "Store"), JSON.stringify(found).slice(0, 160));
  assert.ok(found[0].location.uri.endsWith("Store.ets"));
});

test("a dependency path in hvolt.toml is a link to that module", { skip }, async () => {
  const { root, index, app } = project();
  // The file is deliberately not opened in a buffer: links arrive for any document.
  const answers = await ask(root, index, [
    { id: 2, method: "textDocument/documentLink", params: { textDocument: { uri: `file://${app}` } } },
  ]);
  const links = answers.get(2) ?? [];
  assert.equal(links.length, 1, JSON.stringify(links).slice(0, 160));
  assert.ok(links[0].target.endsWith("/lib"), `target was ${links[0].target}`);
});
