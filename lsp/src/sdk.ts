// Locating the SDK's TypeScript fork. The CLI resolves the SDK and passes it in
// (--sdk), so this module only validates and loads.
import { createRequire } from "node:module";

// The bundle is CommonJS (vscode-languageserver is), where `require` exists and
// `import.meta.url` does not; a source build under ESM is the other way round.
declare const require: ((id: string) => unknown) | undefined;
const loadModule: (id: string) => any =
  typeof require === "function" ? (require as (id: string) => any) : createRequire(import.meta.url);
import { existsSync, readFileSync, readdirSync, statSync } from "node:fs";
import { join } from "node:path";

export type TsModule = any;

const FORK_ENTRY = "build-tools/ets-loader/node_modules/typescript/lib/tsserverlibrary.js";

/** Load the fork from an SDK root (either the store root or a version dir). */
export function loadTypeScript(sdkRoot: string): TsModule {
  const candidates = [
    join(sdkRoot, FORK_ENTRY),
    join(sdkRoot, "ets", FORK_ENTRY),
    join(sdkRoot, "default/openharmony/ets", FORK_ENTRY),
  ];
  for (const entry of candidates) {
    if (existsSync(entry)) {
      return loadModule(entry);
    }
  }
  throw new Error(
    `no TypeScript fork under ${sdkRoot} (looked for ${FORK_ENTRY}); ` +
      `run \`hvolt sdk download <version>\` or point --sdk at an installed SDK`,
  );
}

/** Declaration roots the language service needs: SDK API, ArkUI components, kits. */
export function declarationDirs(sdkRoot: string): string[] {
  const ets = ["ets", "default/openharmony/ets", ""].map((rel) => join(sdkRoot, rel));
  const base = ets.find((dir) => existsSync(join(dir, "oh-uni-package.json")) || existsSync(join(dir, "api")));
  if (!base) return [];
  return ["api", "component", "kits", "arkts"].map((sub) => join(base, sub)).filter((dir) => existsSync(dir));
}

/** Read every declaration file under the given directories. */
export function readDeclarations(dirs: string[]): Map<string, string> {
  const files = new Map<string, string>();
  const walk = (dir: string) => {
    for (const name of readdirSync(dir)) {
      const path = join(dir, name);
      const st = statSync(path);
      if (st.isDirectory()) walk(path);
      else if (/\.d\.(ts|ets)$/.test(name)) files.set(path, readFileSync(path, "utf8"));
    }
  };
  for (const dir of dirs) walk(dir);
  return files;
}
