// Bundles the server into one file: the CLI embeds or points at this single artefact,
// so no node_modules is needed at runtime.
import { build } from "esbuild";
import { mkdirSync } from "node:fs";

mkdirSync("dist", { recursive: true });
await build({
  entryPoints: ["src/server.ts"],
  outfile: "dist/server.cjs",
  bundle: true,
  platform: "node",
  format: "cjs", // vscode-languageserver is CJS; an ESM bundle would stub out require()
  target: "node18",
  banner: { js: "#!/usr/bin/env node" },
  logLevel: "info",
});
