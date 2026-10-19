// Bundles the server into one file: the CLI embeds or points at this single artefact,
// so no node_modules is needed at runtime.
import { build } from "esbuild";
import { mkdirSync } from "node:fs";

mkdirSync("dist", { recursive: true });
// The diagnostics module is also built on its own so tests can import it.
await build({
  entryPoints: ["src/arkui.ts"],
  outfile: "dist/arkui.mjs",
  bundle: true,
  platform: "node",
  format: "esm",
  external: ["typescript"],
});

await build({
  entryPoints: ["src/diagnostics.ts"],
  outfile: "dist/diagnostics.mjs",
  bundle: true,
  platform: "node",
  format: "esm",
  target: "node18",
});

await build({
  entryPoints: ["src/hvolttoml.ts"],
  outfile: "dist/hvolttoml.mjs",
  bundle: true,
  platform: "node",
  format: "esm",
  target: "node18",
});

await build({
  entryPoints: ["src/commands.ts"],
  outfile: "dist/commands.mjs",
  bundle: true,
  platform: "node",
  format: "esm",
  target: "node18",
});

await build({
  entryPoints: ["src/tomlcheck.ts"],
  outfile: "dist/tomlcheck.mjs",
  bundle: true,
  platform: "node",
  format: "esm",
  target: "node18",
});

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
