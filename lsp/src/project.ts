// The hvolt project: flat tree, so sources are whatever .ets/.ts files sit under
// the workspace, minus build output and dependencies.
import { readdirSync, readFileSync, statSync } from "node:fs";
import { join } from "node:path";

const SKIP = new Set([".git", ".hvolt", "build", "output", "node_modules", "oh_modules", "target", ".venv"]);

export function collectSources(root: string): Map<string, string> {
  const files = new Map<string, string>();
  const walk = (dir: string) => {
    let entries: string[];
    try {
      entries = readdirSync(dir);
    } catch {
      return;
    }
    for (const name of entries) {
      if (SKIP.has(name)) continue;
      const path = join(dir, name);
      let st;
      try {
        st = statSync(path);
      } catch {
        continue;
      }
      if (st.isDirectory()) walk(path);
      else if (/\.(ets|ts)$/.test(name) && !name.endsWith(".d.ts")) files.set(path, readFileSync(path, "utf8"));
    }
  };
  walk(root);
  return files;
}
