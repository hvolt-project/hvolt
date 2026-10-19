// Drives the server over LSP for the hvolt.toml features: dependency definition and
// route validation. Needs an SDK (the server loads the language service at start),
// so it skips without HVOLT_SDK.
import { spawn } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const sdk = process.env.HVOLT_SDK;
const lspDir = fileURLToPath(new URL("..", import.meta.url));
const root = process.env.HVOLT_TEST_ROOT ?? join(lspDir, "..", "examples", "hello-native");
if (!sdk || !existsSync(sdk)) {
  console.log("  skip: HVOLT_SDK not set");
  process.exit(0);
}

const file = join(root, "hvolt.toml");
const text = readFileSync(file, "utf8");
const uri = "file://" + file;
const srv = spawn(process.execPath, [join(lspDir, "dist", "server.cjs"), "--sdk", sdk, "--root", root], {
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
const wait = async (predicate, ms) => {
  const until = Date.now() + ms;
  while (Date.now() < until) {
    const hit = predicate();
    if (hit) return hit;
    await new Promise((r) => setTimeout(r, 200));
  }
  return null;
};

await send("initialize", { processId: process.pid, rootUri: "file://" + root, capabilities: {} });
notify("initialized", {});
notify("textDocument/didOpen", { textDocument: { uri, languageId: "toml", version: 1, text } });

// 1) dependency definition
const depLine = text.split("\n").findIndex((l) => l.includes("path ="));
const depColumn = text.split("\n")[depLine].indexOf('"') + 3;
const def = await send("textDocument/definition", { textDocument: { uri }, position: { line: depLine, character: depColumn } });
const target = def.result?.[0]?.uri ?? "";
console.log("  dependency definition →", target.split("/").slice(-2).join("/"));
const defOk = target.endsWith("/mylib/hvolt.toml");

// 2) route validation, on the module file that actually declares routes
const moduleFile = join(root, "src", "hvolt.toml");
const moduleUri = "file://" + moduleFile;
const moduleText = readFileSync(moduleFile, "utf8");
notify("textDocument/didOpen", { textDocument: { uri: moduleUri, languageId: "toml", version: 1, text: moduleText } });
const broken = moduleText.replace(/(routes\s*=\s*\[)/, '$1\n    "pages/NoSuchPage.ets",');
notify("textDocument/didChange", { textDocument: { uri: moduleUri, version: 2 }, contentChanges: [{ text: broken }] });
const reported = await wait(
  () => diagnostics.find((d) => d.uri.endsWith("/src/hvolt.toml") && d.diagnostics.some((x) => /does not exist/.test(x.message))),
  15000,
);
console.log("  route validation →", reported ? `L${reported.diagnostics[0].range.start.line + 1}: ${reported.diagnostics[0].message}` : "none");

const pass = defOk && Boolean(reported);
console.log(pass ? "  PASS" : "  FAIL");
srv.kill();
process.exit(pass ? 0 : 1);
