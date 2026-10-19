// Validation and navigation for hvolt.toml.
//
// A line scanner is enough: the schema is flat (sections + scalar keys + string
// arrays + inline tables), and it keeps the reported positions exact. Anything the
// scanner cannot understand is left alone rather than guessed at.

import { dirname, join } from "node:path";

export interface TomlDiagnostic {
  line: number; // 0-based, for LSP
  character: number;
  length: number;
  message: string;
  severity: "error" | "warning";
}

export interface Entry {
  section: string;
  key: string;
  raw: string;
  line: number;
  column: number;
}

export const APP_KEYS = new Set([
  "bundle",
  "name",
  "version",
  "vendor",
  "icon",
  "bundle_type",
  "sdk_version",
  "compile_sdk_version",
  "compatible_sdk_version",
  "runtime_os",
  "products",
  "members",
  "sdk_path",
  "signing",
]);

export const MODULE_KEYS = new Set([
  "name",
  "entry",
  "routes",
  "device_types",
  "skills",
  "abilities",
  "permissions",
  "obfuscation",
  "abi_filters",
  "type",
]);

/** Flat `section.key = value` entries, with their positions. */
export function scan(text: string): Entry[] {
  const entries: Entry[] = [];
  const lines = text.split(/\r?\n/);
  let section = "";
  for (let index = 0; index < lines.length; index++) {
    const startLine = index;
    const lineText = lines[index];
    const trimmed = lineText.trim();
    if (!trimmed || trimmed.startsWith("#")) continue;

    const header = /^\[([A-Za-z_]+)\]\s*$/.exec(trimmed);
    if (header) {
      section = header[1];
      continue;
    }
    const pair = /^([A-Za-z_][A-Za-z0-9_]*)\s*=\s*(.*)$/.exec(trimmed);
    if (!pair) continue;

    // Arrays may span several lines (the examples write device_types that way), so
    // keep consuming until the brackets balance before validating the value.
    let raw = pair[2];
    if (raw.startsWith("[") && bracketDepth(raw) > 0) {
      while (index + 1 < lines.length && bracketDepth(raw) > 0) {
        index++;
        raw += "\n" + lines[index];
      }
    }
    entries.push({ section, key: pair[1], raw, line: startLine, column: lineText.indexOf(pair[1]) });
  }
  return entries;
}

/** 1 while an array literal is still open, 0 once closed. */
function bracketDepth(raw: string): number {
  let depth = 0;
  let inString = false;
  for (const ch of raw) {
    if (ch === '"') inString = !inString;
    else if (!inString && ch === "[") depth++;
    else if (!inString && ch === "]") depth--;
  }
  return depth;
}

export interface ValidateOptions {
  /** Resolve a route to the file it names; defaults to "assume it exists". */
  routeExists?: (route: string) => boolean;
}

export function validateToml(text: string, options: ValidateOptions = {}): TomlDiagnostic[] {
  const diagnostics: TomlDiagnostic[] = [];
  const allowed = (section: string) => (section === "" || section === "app" ? APP_KEYS : section === "module" ? MODULE_KEYS : null);

  for (const entry of scan(text)) {
    const keys = allowed(entry.section);
    if (keys && !keys.has(entry.key)) {
      diagnostics.push({
        line: entry.line,
        character: entry.column,
        length: entry.key.length,
        message: `unknown key '${entry.key}' in [${entry.section || "app"}]`,
        severity: "warning",
      });
    }
    if (entry.key === "routes") {
      for (const route of quoted(entry.raw)) {
        if (options.routeExists && !options.routeExists(route)) {
          diagnostics.push({
            line: entry.line,
            character: entry.column,
            length: entry.key.length,
            message: `route '${route}' does not exist in this module`,
            severity: "error",
          });
        }
      }
    }
    if (entry.key === "device_types" && !entry.raw.trim().startsWith("[")) {
      diagnostics.push({
        line: entry.line,
        character: entry.column,
        length: entry.key.length,
        message: "device_types must be an array",
        severity: "error",
      });
    }
  }
  return diagnostics;
}

function quoted(raw: string): string[] {
  return [...raw.matchAll(/"([^"]*)"/g)].map((match) => match[1]);
}

/** The dependency path whose string literal the cursor sits in, if any. */
export function dependencyAt(text: string, line: number, character: number): string | null {
  const lineText = text.split(/\r?\n/)[line] ?? "";
  const pair = /^([A-Za-z_][A-Za-z0-9_-]*)\s*=\s*\{\s*path\s*=\s*"([^"]*)"/.exec(lineText.trim());
  if (!pair) return null;
  const open = lineText.indexOf('"') + 1;
  const close = lineText.indexOf('"', open);
  if (character < open || character > close) return null;
  return pair[2];
}

/** Where a dependency's path points, from the toml file's own directory. */
export function dependencyTarget(tomlPath: string, path: string): string {
  return join(dirname(tomlPath), path, "hvolt.toml");
}
