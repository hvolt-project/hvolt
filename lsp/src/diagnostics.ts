// Diagnostics for hvolt projects.
//
// Two sources, both authoritative in the sense that they are what a build or the
// official linter reports — unlike the SDK fork's own checker, which mis-reports
// legal ArkUI block-chains (see DESIGN.md). Both report paths in generated trees,
// so everything is mapped back to the flat project layout first.

import { isAbsolute, join, relative, sep } from "node:path";

export interface Diagnostic {
  path: string;
  line: number; // 1-based
  column: number; // 1-based
  message: string;
  severity: "error" | "warning" | "info";
  source: string;
}

const LINT_TREE = /([\\/])\.hvolt[\\/]lint[\\/]tree[\\/][^\\/]+[\\/]/;

/** Map a path from a generated tree (lint copy, build output) to the flat project. */
export function mapToProject(file: string, root: string): string | null {
  const normalized = file.replace(/\\/g, "/");

  const lintMatch = LINT_TREE.exec(normalized);
  if (lintMatch) {
    const inside = normalized.slice(lintMatch.index + lintMatch[0].length);
    return flattenModulePath(inside, root);
  }

  const normalizedRoot = root.replace(/\\/g, "/").replace(/\/$/, "");
  if (normalized.startsWith(normalizedRoot + "/build/")) {
    const inside = normalized.slice(normalizedRoot.length + "/build/".length);
    return flattenModulePath(inside, root);
  }

  if (isAbsolute(normalized) && normalized.startsWith(normalizedRoot + "/")) {
    return normalized;
  }
  return null;
}

/** `<module>/src/main/ets/<rel>` → `<module>/<rel>`, which is the flat layout. */
function flattenModulePath(inside: string, root: string): string {
  const marker = "/src/main/ets/";
  const at = inside.indexOf(marker);
  if (at < 0) return join(root, inside);
  return join(root, inside.slice(0, at), inside.slice(at + marker.length));
}

/** hvigor's diagnostics: `Error Message: <text>. At File: <path>:<line>:<col>`. */
export function parseHvigor(text: string, root: string): Diagnostic[] {
  const out: Diagnostic[] = [];
  const pattern = /Error Message: ([\s\S]*?)At File: ([^\s:]+(?::[A-Za-z]:)?[^\s]*?):(\d+):(\d+)/g;
  for (const match of text.matchAll(pattern)) {
    const [, rawMessage, file, line, column] = match;
    const mapped = mapToProject(file, root);
    if (!mapped) continue;
    out.push({
      path: mapped,
      line: Number(line),
      column: Number(column),
      message: rawMessage.trim().replace(/\.$/, ""),
      severity: "error",
      source: "hvigor",
    });
  }
  return out;
}

interface LintDefect {
  file: string;
  line?: number;
  column?: number;
  message?: string;
  rule?: string;
  severity?: number;
}

/** codelinter's report (`hvolt lint --json`); severity 1/2/3 = error/warning/info. */
export function fromLint(json: unknown, root: string): Diagnostic[] {
  const defects = (json as { defects?: LintDefect[] } | null)?.defects ?? [];
  const out: Diagnostic[] = [];
  for (const defect of defects) {
    if (!defect.file) continue;
    const mapped = mapToProject(defect.file, root);
    if (!mapped) continue;
    out.push({
      path: mapped,
      line: defect.line ?? 1,
      column: defect.column ?? 1,
      message: defect.rule ? `${defect.message ?? "defect"} (${defect.rule})` : (defect.message ?? "defect"),
      severity: defect.severity === 1 ? "error" : defect.severity === 3 ? "info" : "warning",
      source: "codelinter",
    });
  }
  return out;
}

/** Relative path for logging and tests. */
export function relPath(path: string, root: string): string {
  return relative(root, path).split(sep).join("/");
}
