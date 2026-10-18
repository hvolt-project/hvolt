// Drives the built server over LSP and checks the three capabilities M1 promises.
// Needs an SDK: set HVOLT_SDK, or it exits 0 with a notice (CI has no SDK).
import { spawn } from "node:child_process";
import { readFileSync, existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { join } from "node:path";

const sdk = process.env.HVOLT_SDK;
const lspDir = fileURLToPath(new URL("..", import.meta.url));
const repoRoot = fileURLToPath(new URL("../..", import.meta.url));
const root = process.env.HVOLT_TEST_ROOT ?? join(repoRoot, "examples", "hello-world");
if (!sdk || !existsSync(sdk)) {
  console.log("  skip: HVOLT_SDK not set — no SDK on this machine");
  process.exit(0);
}

const file = join(root, "src", "pages", "Index.ets");
const text = readFileSync(file, "utf8");
const uri = "file://" + file;

const srv = spawn("node", [join(lspDir, "dist", "server.cjs"), "--sdk", sdk, "--root", root], {
  stdio: ["pipe", "pipe", "inherit"],
});

let buf = Buffer.alloc(0);
const pending = new Map();
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
    if (msg.id && pending.has(msg.id)) {
      pending.get(msg.id)(msg);
      pending.delete(msg.id);
    }
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

const t0 = Date.now();
const init = await send("initialize", { processId: process.pid, rootUri: "file://" + root, capabilities: {} });
console.log("  initialize:", init.result?.serverInfo?.name, "elapsed", Date.now() - t0, "ms");
notify("initialized", {});
notify("textDocument/didOpen", { textDocument: { uri, languageId: "ets", version: 1, text } });

const lines = text.split("\n");
const findPos = (needle) => {
  const line = lines.findIndex((l) => l.includes(needle));
  return { line, character: lines[line].indexOf(needle) + 1 };
};

const hoverPos = findPos("RelativeContainer(");
const hover = await send("textDocument/hover", { textDocument: { uri }, position: hoverPos });
const hoverText = hover.result?.contents?.value ?? "";
console.log("  hover(RelativeContainer):", hoverText.split("\n")[1]?.slice(0, 70));
if (!hoverText.includes("RelativeContainer")) {
  console.error("  FAIL: hover did not resolve RelativeContainer to its interface");
  process.exit(1);
}

const textPos = findPos("Text(this.message)");
const definition = await send("textDocument/definition", { textDocument: { uri }, position: textPos });
const loc = definition.result?.[0];
console.log("  definition(Text):", loc?.uri?.split("/").slice(-2).join("/"), `L${loc?.range?.start?.line}`);
if (!loc?.uri?.includes("/component/")) {
  console.error("  FAIL: definition did not resolve into the SDK component declarations");
  process.exit(1);
}

const completion = await send("textDocument/completion", { textDocument: { uri }, position: { line: hoverPos.line, character: hoverPos.character + 2 } });
const items = completion.result ?? [];
console.log("  completion:", items.length, "items; sample:", items.slice(0, 3).map((i) => i.label).join(", "));
if (items.length < 100) {
  console.error("  FAIL: too few completions");
  process.exit(1);
}

const elapsed = Date.now() - t0;
const rss = process.memoryUsage().rss / 1e6;
console.log(`  elapsed ${elapsed} ms, client rss ${rss.toFixed(0)} MB`);
srv.kill();
