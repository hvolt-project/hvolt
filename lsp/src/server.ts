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
import { collectSources } from "./project.js";
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
const connection = createConnection(ProposedFeatures.all, process.stdin, process.stdout);
const documents = new TextDocuments(TextDocument);

connection.onInitialize(() => ({
  serverInfo: { name: "hvolt-lsp" },
  capabilities: {
    textDocumentSync: TextDocumentSyncKind.Full,
    hoverProvider: true,
    completionProvider: { resolveProvider: false, triggerCharacters: [".", "@", "'", "/"] },
    definitionProvider: true,
    signatureHelpProvider: { triggerCharacters: ["(", ","] },
    renameProvider: true,
  },
}));

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

documents.listen(connection);
connection.listen();
