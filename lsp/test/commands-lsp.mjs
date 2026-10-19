// Drives `hvolt.build` through the server and checks the contract the extension
// relies on: a structured result plus a window/showMessage notification. Whether the
// build itself succeeds depends on the machine (hvigor/SDK), so that is not asserted.
import { spawn } from "node:child_process";
import { existsSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const lspDir = fileURLToPath(new URL("..", import.meta.url));
const root = process.env.HVOLT_TEST_ROOT ?? join(lspDir, "..", "examples", "hello-world");
const sdk = process.env.HVOLT_SDK;
const bin = process.env.HVOLT_BIN ?? join(lspDir, "..", "target", "debug", "hvolt");
if (!sdk || !existsSync(bin)) {
  console.log("  skip: HVOLT_SDK or hvolt binary missing");
  process.exit(0);
}

const srv = spawn(process.execPath, [join(lspDir, "dist", "server.cjs"), "--sdk", sdk, "--root", root], {
  stdio: ["pipe", "pipe", "inherit"],
});
let buf = Buffer.alloc(0);
const pending = new Map();
const notifications = [];
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
    } else if (msg.method) {
      notifications.push(msg);
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

await send("initialize", {
  processId: process.pid,
  rootUri: "file://" + root,
  capabilities: {},
  initializationOptions: { hvolt: { binaryPath: bin, lintOnSave: false } },
});
notify("initialized", {});

const total = await send("workspace/executeCommand", { command: "hvolt.executeCli", arguments: [root, "hvolt.build"] });
await new Promise((r) => setTimeout(r, 1200));

const result = total.result;
const message = notifications.find((n) => n.method === "window/showMessage");
console.log("  result:", JSON.stringify(result)?.slice(0, 100) ?? "(none)");
console.log("  notification:", message ? message.params.message : "(none)");

const ok = Boolean(result && typeof result.seconds === "string" && message && /^hvolt build →/.test(message.params.message));
console.log(ok ? "  PASS" : "  FAIL");
srv.kill();
process.exit(ok ? 0 : 1);
