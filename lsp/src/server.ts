// hvolt language server.
//
// Semantics come from the TypeScript fork inside the user's SDK (it parses .ets and
// ArkUI structs); diagnostics deliberately do not — the fork mis-reports legal
// ArkUI block-chains, so diagnostics are left to hvigor, codelinter and
// app_check_tool. See lsp/DESIGN.md.
import {
  createConnection,
  CompletionItemKind,
  MarkupKind,
  ProposedFeatures,
  TextDocuments,
  TextDocumentSyncKind,
} from "vscode-languageserver/node.js";
import { TextDocument } from "vscode-languageserver-textdocument";
import { execFile } from "node:child_process";
import { collectSources } from "./project.js";
import { type Diagnostic, fromLint, parseHvigor } from "./diagnostics.js";
import { completeKeys, declaredRoutes, inRouterCall } from "./hvolttoml.js";
import { dependencyAt, dependencyTarget, validateToml } from "./tomlcheck.js";
import { existsSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { declarationDirs, loadTypeScript, readDeclarations } from "./sdk.js";

const args = process.argv.slice(2);
const option = (name: string): string | undefined => {
  const i = args.indexOf(name);
  return i >= 0 ? args[i + 1] : undefined;
};
const sdkRoot = option("--sdk") ?? process.env.HVOLT_SDK;
const rootDir = option("--root") ?? process.cwd();

if (!sdkRoot) {
  // A server without an SDK cannot answer anything; say so once and exit cleanly.
  process.stderr.write("hvolt-lsp: no SDK given (--sdk <path> or HVOLT_SDK); nothing to serve\n");
  process.exit(0);
}

const ts = loadTypeScript(sdkRoot);
const root = process.env.HVOLT_LSP_ROOT ?? rootDir;
const files = readDeclarations(declarationDirs(sdkRoot));
for (const [path, text] of collectSources(root)) files.set(path, text);
const versions = new Map<string, number>();

const host = {
  getScriptFileNames: () => [...files.keys()],
  getScriptVersion: (file: string) => String(versions.get(file) ?? 0),
  getScriptSnapshot: (file: string) => {
    const text = files.get(file);
    return text === undefined ? undefined : ts.ScriptSnapshot.fromString(text);
  },
  getCurrentDirectory: () => root,
  getCompilationSettings: () => ({
    target: ts.ScriptTarget.ES2021,
    module: ts.ModuleKind.ES2021,
    moduleResolution: ts.ModuleResolutionKind.NodeJs,
    experimentalDecorators: true,
    strict: true,
    allowNonTsExtensions: true,
  }),
  getDefaultLibFileName: (options: any) =>
    `${sdkRoot}/build-tools/ets-loader/node_modules/typescript/lib/${ts.getDefaultLibFileName(options)}`,
  fileExists: (file: string) => files.has(file),
  readFile: (file: string) => files.get(file),
  getScriptKind: (file: string) => (file.endsWith(".ets") ? (ts.ScriptKind.ETS ?? 8) : undefined),
};

const service = ts.createLanguageService(host, ts.createDocumentRegistry());
const positionOf = (file: string, offset: number) => {
  const text = files.get(file) ?? "";
  const source = ts.createSourceFile(file, text, ts.ScriptTarget.ES2021);
  return source.getLineAndCharacterOfPosition(offset);
};
const offsetOf = (file: string, line: number, character: number) => {
  const text = files.get(file) ?? "";
  const source = ts.createSourceFile(file, text, ts.ScriptTarget.ES2021);
  return source.getPositionOfLineAndCharacter(line, character);
};

// Explicit streams: the transport never comes from CLI flags, so the server can
// take its own --sdk/--root options.
interface HvoltSettings {
  binaryPath?: string;
  lintOnSave?: boolean;
  /** Run codelinter with its broad rule sets (@performance/all + @typescript-eslint/all). */
  lintAllRules?: boolean;
}
let settings: HvoltSettings = {};
const binaryPath = () => settings.binaryPath ?? process.env.HVOLT_BIN ?? "hvolt";

const connection = createConnection(ProposedFeatures.all, process.stdin, process.stdout);
const documents = new TextDocuments(TextDocument);

connection.onInitialize((params) => {
  settings = (params.initializationOptions?.hvolt ?? {}) as HvoltSettings;
  return {
    serverInfo: { name: "hvolt-lsp" },
    capabilities: {
      textDocumentSync: TextDocumentSyncKind.Full,
      hoverProvider: true,
      completionProvider: { resolveProvider: false, triggerCharacters: [".", "@", "'", "/"] },
      definitionProvider: true,
      signatureHelpProvider: { triggerCharacters: ["(", ","] },
      renameProvider: true,
      executeCommandProvider: { commands: ["hvolt.buildDiagnostics"] },
    },
  };
});

documents.onDidOpen((e) => {
  files.set(e.document.uri.replace(/^file:\/\//, ""), e.document.getText());
  versions.set(e.document.uri.replace(/^file:\/\//, ""), e.document.version);
});
documents.onDidChangeContent((e) => {
  const path = e.document.uri.replace(/^file:\/\//, "");
  files.set(path, e.document.getText());
  versions.set(path, e.document.version);
});

connection.onHover(({ textDocument, position }) => {
  const file = textDocument.uri.replace(/^file:\/\//, "");
  const info = service.getQuickInfoAtPosition(file, offsetOf(file, position.line, position.character));
  if (!info) return null;
  const text = ts.displayPartsToString(info.displayParts ?? []);
  const docs = ts.displayPartsToString(info.documentation ?? []);
  return {
    contents: { kind: MarkupKind.Markdown, value: "```ts\n" + text + "\n```" + (docs ? "\n\n" + docs : "") },
    range: undefined,
  };
});

connection.onCompletion(({ textDocument, position }) => {
  const file = textDocument.uri.replace(/^file:\/\//, "");
  const text = files.get(file) ?? "";
  const lineText = (text.split(/\r?\n/)[position.line] ?? "");

  // hvolt.toml: complete the documented keys of the section under the cursor.
  if (file.endsWith("hvolt.toml")) {
    return completeKeys(text, position.line).map((entry) => ({
      label: entry.key,
      kind: CompletionItemKind.Property,
      detail: entry.detail,
    }));
  }

  // `router.pushUrl('pages/…')`: complete the routes the module declares.
  if (inRouterCall(lineText)) {
    const moduleDir = file.slice(0, file.lastIndexOf("/pages/")) || file.replace(/\/[^/]+$/, "");
    const moduleToml = files.get(moduleDir + "/hvolt.toml") ?? "";
    return declaredRoutes(moduleToml).map((route) => ({
      label: route,
      kind: CompletionItemKind.File,
      detail: "route declared in hvolt.toml",
    }));
  }
  const completions = service.getCompletionsAtPosition(file, offsetOf(file, position.line, position.character), {});
  if (!completions) return [];
  return completions.entries.map((entry: any) => ({
    label: entry.name,
    kind: completionKind(ts, entry.kind),
    detail: entry.kind,
    sortText: entry.sortText,
  }));
});

connection.onDefinition(({ textDocument, position }) => {
  const file = textDocument.uri.replace(/^file:\/\//, "");

  // Inside `lib = { path = "../lib" }`: jump to that module's hvolt.toml.
  if (file.endsWith("hvolt.toml")) {
    const text = files.get(file) ?? "";
    const dependency = dependencyAt(text, position.line, position.character);
    if (!dependency) return [];
    const target = dependencyTarget(file, dependency);
    return [
      {
        uri: "file://" + target,
        range: { start: { line: 0, character: 0 }, end: { line: 0, character: 0 } },
      },
    ];
  }
  const defs = service.getDefinitionAtPosition(file, offsetOf(file, position.line, position.character)) ?? [];
  return defs.map((def: any) => {
    const start = positionOf(def.fileName, def.textSpan.start);
    const end = positionOf(def.fileName, def.textSpan.start + def.textSpan.length);
    return {
      uri: "file://" + def.fileName,
      range: { start: { line: start.line, character: start.character }, end: { line: end.line, character: end.character } },
    };
  });
});

connection.onSignatureHelp(({ textDocument, position }) => {
  const file = textDocument.uri.replace(/^file:\/\//, "");
  const help = service.getSignatureHelpItems(file, offsetOf(file, position.line, position.character), {});
  if (!help) return null;
  return {
    activeSignature: help.selectedItemIndex ?? 0,
    activeParameter: help.argumentIndex ?? 0,
    signatures: help.items.map((item: any) => ({
      label: ts.displayPartsToString(item.prefix) + ts.displayPartsToString(item.suffix),
      documentation: { kind: MarkupKind.Markdown, value: ts.displayPartsToString(item.documentation ?? []) },
    })),
  };
});

connection.onRenameRequest(({ textDocument, position, newName }) => {
  const file = textDocument.uri.replace(/^file:\/\//, "");
  const rename = service.findRenameLocations(file, offsetOf(file, position.line, position.character), false, false) ?? [];
  const changes: Record<string, any[]> = {};
  for (const loc of rename) {
    const start = positionOf(loc.fileName, loc.textSpan.start);
    const end = positionOf(loc.fileName, loc.textSpan.start + loc.textSpan.length);
    const uri = "file://" + loc.fileName;
    (changes[uri] ??= []).push({
      range: { start: { line: start.line, character: start.character }, end: { line: end.line, character: end.character } },
      newText: newName,
    });
  }
  return { changes };
});

function completionKind(tsModule: any, kind: string): number {
  const map: Record<string, number> = {
    keyword: CompletionItemKind.Keyword,
    const: CompletionItemKind.Constant,
    let: CompletionItemKind.Variable,
    var: CompletionItemKind.Variable,
    method: CompletionItemKind.Method,
    property: CompletionItemKind.Property,
    function: CompletionItemKind.Function,
    class: CompletionItemKind.Class,
    interface: CompletionItemKind.Interface,
    type: CompletionItemKind.TypeParameter,
  };
  return map[kind] ?? CompletionItemKind.Text;
}


// ── diagnostics ───────────────────────────────────────────────────────────
// Sources are the real tools (see DESIGN.md); the fork's checker is not consulted.

// Diagnostics are kept per source and merged for publication: a build result must
// not wipe the linter's findings for the same file.
const bySource = new Map<string, Diagnostic[]>();
const published = new Set<string>();

function flush(): void {
  const merged = new Map<string, Diagnostic[]>();
  for (const list of bySource.values()) {
    for (const diagnostic of list) {
      const bucket = merged.get(diagnostic.path) ?? [];
      bucket.push(diagnostic);
      merged.set(diagnostic.path, bucket);
    }
  }
  for (const path of published) {
    if (!merged.has(path)) {
      connection.sendDiagnostics({ uri: "file://" + path, diagnostics: [] });
      published.delete(path);
    }
  }
  for (const [path, list] of merged) {
    connection.sendDiagnostics({
      uri: "file://" + path,
      diagnostics: list.map((diagnostic) => ({
        range: {
          start: { line: Math.max(0, diagnostic.line - 1), character: Math.max(0, diagnostic.column - 1) },
          end: { line: Math.max(0, diagnostic.line - 1), character: Math.max(0, diagnostic.column) },
        },
        message: diagnostic.message,
        severity: diagnostic.severity === "error" ? 1 : diagnostic.severity === "warning" ? 2 : 3,
        source: diagnostic.source,
      })),
    });
    published.add(path);
  }
}

function publish(source: string, diagnostics: Diagnostic[]): void {
  bySource.set(source, diagnostics);
  flush();
}

function run(binary: string, args: string[], cwd: string): Promise<string> {
  return new Promise((resolve) => {
    // Both streams matter: hvigor writes progress to stdout and the diagnostics to
    // stderr, so the two are concatenated rather than picking one.
    execFile(binary, args, { cwd, maxBuffer: 64 * 1024 * 1024 }, (_error, stdout, stderr) => {
      resolve(`${stdout ?? ""}${stderr ?? ""}`);
    });
  });
}

let lintTimer: NodeJS.Timeout | undefined;
function lintSoon(): void {
  if (settings.lintOnSave === false) return;
  clearTimeout(lintTimer);
  lintTimer = setTimeout(async () => {
    const args = ["lint", "--json"];
    if (settings.lintAllRules) args.push("--all-rules");
    const output = await run(binaryPath(), args, root);
    if (process.env.HVOLT_LSP_DEBUG) {
      process.stderr.write(`hvolt-lsp: lint ${args.join(" ")} → ${output.length} bytes\n`);
    }
    // `--json` writes one JSON document, but stderr carries human progress lines
    // that may also start with "{", so the last JSON line wins.
    const report = output
      .split(/\r?\n/)
      .filter((line) => line.trim().startsWith("{"))
      .map((line) => {
        try {
          return JSON.parse(line) as Record<string, unknown>;
        } catch {
          return undefined;
        }
      })
      .filter((value): value is Record<string, unknown> => Boolean(value) && "defects" in (value as object))
      .pop();
    if (!report) {
      if (process.env.HVOLT_LSP_DEBUG) process.stderr.write("hvolt-lsp: lint report not found\n");
      return;
    }
    const parsed = fromLint(report, root);
    if (process.env.HVOLT_LSP_DEBUG) process.stderr.write(`hvolt-lsp: lint → ${parsed.length} diagnostics\n`);
    publish("lint", parsed);
  }, 400);
}

function validateTomlDocument(path: string, text: string): void {
  if (!path.endsWith("hvolt.toml")) return;
  const moduleDir = dirname(path);
  const diagnostics = validateToml(text, {
    routeExists: (route) => existsSync(join(moduleDir, route)),
  });
  bySource.set(
    "hvolt.toml",
    diagnostics.map((diagnostic) => ({
      path,
      line: diagnostic.line + 1,
      column: diagnostic.character + 1,
      message: diagnostic.message,
      severity: diagnostic.severity,
      source: "hvolt.toml",
    })),
  );
  flush();
}

documents.onDidOpen((e) => validateTomlDocument(e.document.uri.replace(/^file:\/\//, ""), e.document.getText()));
documents.onDidChangeContent((e) => validateTomlDocument(e.document.uri.replace(/^file:\/\//, ""), e.document.getText()));

documents.onDidOpen(() => lintSoon());
documents.onDidSave(() => lintSoon());

connection.onExecuteCommand(async ({ command, arguments: args }) => {
  if (command !== "hvolt.buildDiagnostics") return null;
  const cwd = (args?.[0] as string | undefined) ?? root;
  const output = await run(binaryPath(), ["build"], cwd);
  const parsed = parseHvigor(output, root);
  if (process.env.HVOLT_LSP_DEBUG) {
    process.stderr.write(`hvolt-lsp: build in ${cwd} → ${output.length} bytes, ${parsed.length} diagnostics\n`);
  }
  publish("build", parsed);
  return null;
});

documents.listen(connection);
connection.listen();
