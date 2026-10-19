// End-to-end diagnostics check against a real project + real toolchain.
// Needs HVOLT_BIN (a hvolt binary), HVOLT_SDK and HVOLT_TEST_ROOT (a project with
// hvigor/codelinter available). Skips otherwise.
import { spawn } from "node:child_process";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const bin = process.env.HVOLT_BIN;
const sdk = process.env.HVOLT_SDK;
const root = process.env.HVOLT_TEST_ROOT;
if (!bin || !sdk || !root || !existsSync(join(root, "hvolt.toml"))) {
  console.log("  skip: HVOLT_BIN/HVOLT_SDK/HVOLT_TEST_ROOT not set");
  process.exit(0);
}

const rel = process.env.HVOLT_TEST_FILE ?? "src/pages/Index.ets";
const file = join(root, rel);
const original = readFileSync(file, "utf8");
const uri = "file://" + file;

const srv = spawn(process.execPath, [join(fileURLToPath(new URL("..", import.meta.url)), "dist", "server.cjs"), "--sdk", sdk, "--root", root], {
  stdio: ["pipe", "pipe", "inherit"],
});
let buf = Buffer.alloc(0);
const pending = new Map();
const diagnostics = [];
srv.stdout.on("data", (chunk) => {
  buf = Buffer.concat([buf, chunk]);
  for (;;) {
    const sep = buf.indexOf("\r\n\r\n");
    if (sep < 0) break;
    const m = /Content-Length: (\d+)/i.exec(buf.slice(0, sep).toString());
    if (!m) break;
    const len = Number(m[1]);
    const body = buf.slice(sep + 4, sep + 4 + len);
    if (body.length < len) break;
    buf = buf.slice(sep + 4 + len);
    const msg = JSON.parse(body.toString());
    if (msg.id && pending.has(msg.id)) { pending.get(msg.id)(msg); pending.delete(msg.id); }
    else if (msg.method === "textDocument/publishDiagnostics") diagnostics.push(msg.params);
  }
});
let id = 0;
const send = (method, params) => {
  const msg = JSON.stringify({ jsonrpc: "2.0", id: ++id, method, params });
  srv.stdin.write(`Content-Length: ${Buffer.byteLength(msg)}\r\n\r\n${msg}`);
  return new Promise((res) => pending.set(id, res));
};
const notify = (method, params) => {
  const msg = JSON.stringify({ jsonrpc: "2.0", method, params });
  srv.stdin.write(`Content-Length: ${Buffer.byteLength(msg)}\r\n\r\n${msg}`);
};
const waitFor = async (predicate, ms) => {
  const until = Date.now() + ms;
  while (Date.now() < until) {
    const hit = predicate();
    if (hit) return hit;
    await new Promise((r) => setTimeout(r, 250));
  }
  return null;
};

await send("initialize", {
  processId: process.pid,
  rootUri: "file://" + root,
  capabilities: {},
  initializationOptions: { hvolt: { binaryPath: bin, lintAllRules: true } },
});
notify("initialized", {});
notify("textDocument/didOpen", { textDocument: { uri, languageId: "ets", version: 1, text: original } });
notify("textDocument/didSave", { textDocument: { uri } });

const lint = await waitFor(() => diagnostics.find((d) => d.diagnostics.length > 0), 60000);
console.log("  lint diagnostics:", lint ? `${lint.diagnostics.length} in ${lint.uri.split("/").slice(-2).join("/")} e.g. ${lint.diagnostics[0].message.slice(0, 50)}` : "none");

writeFileSync(file, original.replace(/Text\(this\.message\)/, "Text(noSuchVariable)"), "utf8");
await send("workspace/executeCommand", { command: "hvolt.buildDiagnostics", arguments: [root] });
const build = await waitFor(
  () => diagnostics.find((d) => d.diagnostics.some((x) => /noSuchVariable/.test(x.message))),
  180000,
);
writeFileSync(file, original, "utf8");
const compiled = build?.diagnostics.find((d) => /noSuchVariable/.test(d.message));
console.log(
  "  build diagnostics:",
  compiled
    ? `${build.uri.split("/").slice(-2).join("/")} L${compiled.range.start.line + 1}:${compiled.range.start.character + 1} — ${compiled.message.slice(0, 50)}`
    : "none",
);

const flat = (u) => !u.includes("/build/") && !u.includes("/.hvolt/");
const pass = Boolean(lint && flat(lint.uri) && build && flat(build.uri));
console.log(pass ? "  PASS" : "  FAIL");
srv.kill();
process.exit(pass ? 0 : 1);
