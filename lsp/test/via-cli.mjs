// Same checks as smoke.mjs, but goes through `hvolt lsp --stdio` — proves the Rust
// launcher resolves SDK/Node/bundle and hands stdio over correctly.
import { spawn } from "node:child_process";
import { readFileSync, existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { join } from "node:path";

const bin = process.env.HVOLT_BIN ?? join(fileURLToPath(new URL("../..", import.meta.url)), "target", "debug", "hvolt");
const lspDir = fileURLToPath(new URL("..", import.meta.url));
const repoRoot = fileURLToPath(new URL("../..", import.meta.url));
const root = process.env.HVOLT_TEST_ROOT ?? join(repoRoot, "examples", "hello-world");
const sdk = process.env.HVOLT_SDK;
if (!sdk || !existsSync(bin)) {
  console.log("  skip: HVOLT_SDK not set or hvolt binary missing");
  process.exit(0);
}

const file = join(root, "src", "pages", "Index.ets");
const text = readFileSync(file, "utf8");
const uri = "file://" + file;
const srv = spawn(bin, ["lsp", "--stdio", "--sdk", sdk], { cwd: root, stdio: ["pipe", "pipe", "inherit"] });

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
    if (msg.id && pending.has(msg.id)) { pending.get(msg.id)(msg); pending.delete(msg.id); }
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
console.log("  initialize via CLI:", init.result?.serverInfo?.name, Date.now() - t0, "ms");
notify("initialized", {});
notify("textDocument/didOpen", { textDocument: { uri, languageId: "ets", version: 1, text } });

const lines = text.split("\n");
const pos = (needle) => {
  const line = lines.findIndex((l) => l.includes(needle));
  return { line, character: lines[line].indexOf(needle) + 1 };
};
const hover = await send("textDocument/hover", { textDocument: { uri }, position: pos("RelativeContainer(") });
const value = hover.result?.contents?.value ?? "";
const def = await send("textDocument/definition", { textDocument: { uri }, position: pos("Text(this.message)") });
console.log("  hover:", value.split("\n")[1]?.slice(0, 60));
console.log("  definition:", def.result?.[0]?.uri?.split("/").slice(-2).join("/"));
const ok = value.includes("RelativeContainerInterface") && (def.result?.[0]?.uri ?? "").includes("/component/");
console.log(ok ? "  PASS" : "  FAIL");
srv.kill();
process.exit(ok ? 0 : 1);
