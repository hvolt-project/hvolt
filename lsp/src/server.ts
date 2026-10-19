// hvolt language server.
//
// Semantics come from the TypeScript fork inside the user's SDK (it parses .ets and
// ArkUI structs); diagnostics deliberately do not — the fork mis-reports legal
// ArkUI block-chains, so diagnostics are left to hvigor, codelinter and
// app_check_tool. See lsp/DESIGN.md.
import {
  createConnection,
  CompletionItemKind,
  type CompletionItem,
  MarkupKind,
  ProposedFeatures,
  TextDocuments,
  TextDocumentSyncKind,
} from "vscode-languageserver/node.js";
import { TextDocument } from "vscode-languageserver-textdocument";
import { execFile } from "node:child_process";
import { collectSources } from "./project.js";
import { type Diagnostic, fromLint, parseHvigor, rangeOf } from "./diagnostics.js";
import { COMMANDS, type CommandRequest, commandSpec } from "./commands.js";
import { completeKeys, declaredRoutes, inRouterCall } from "./hvolttoml.js";
import { dependencyAt, dependencyTarget, validateToml } from "./tomlcheck.js";
import { appendFileSync, existsSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { declarationDirs, etsBase, loadTypeScript, readDeclarations } from "./sdk.js";
import { transformArkts, toOriginal, toTransformed, type Transformed } from "./transform.js";
import {
  buildAttributeIndex,
  buildEnumIndex,
  buildTypeIndex,
  enumAt,
  chainCallAt,
  chainComponent,
  membersOf,
  parameterTypes,
  type AttributeIndex,
  type TypeIndex,
} from "./arkui.js";

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
const etsRoot = etsBase(sdkRoot) ?? sdkRoot;
const root = process.env.HVOLT_LSP_ROOT ?? rootDir;
const files = readDeclarations(declarationDirs(sdkRoot));

/**
 * Project sources: `originals` is what the editor holds and everything we answer with;
 * `transformed` is the same code with ArkUI blocks rewritten so the language service can
 * parse it. Only the service ever sees the transformed text.
 */
const originals = new Map<string, string>();
const transformed = new Map<string, Transformed>();

const SHIM_PATH = join(root, ".hvolt", "arkui-shim.d.ts");
files.set(
  SHIM_PATH,
  "// Injected by the hvolt language server so the ArkUI transform parses.\ndeclare function __arkui(build: () => void): any;\n",
);

/** Recompute the transformed text for one file. */
function refreshTransform(path: string): void {
  const text = originals.get(path);
  if (text === undefined) return;
  const result = transformArkts(text, (name) => attributes().components.has(name));
  transformed.set(path, result);
  files.set(path, result.code);
  log(`transform ${path.replace(root + "/", "")}: changed=${result.changed} ${text.length}→${result.code.length}`);
}

for (const [path, text] of collectSources(root)) {
  originals.set(path, text);
  files.set(path, text);
}
const versions = new Map<string, number>();

/**
 * Files the program may read beyond the in-memory map: the SDK's declaration tree and
 * the compiler's own lib files. Anything else (notably the SDK's node_modules) is
 * refused — resolution happily walks into it and exhausts the heap.
 */
function onDisk(file: string): string | undefined {
  const allowed =
    file.startsWith(etsRoot) ||
    file.includes("ets-loader/node_modules/typescript/lib/") ||
    file.startsWith(root);
  if (!allowed || !existsSync(file)) return undefined;
  try {
    return readFileSync(file, "utf8");
  } catch {
    return undefined;
  }
}

const host = {
  getScriptFileNames: () => [...files.keys()],
  getScriptVersion: (file: string) => String(versions.get(file) ?? 0),
  getScriptSnapshot: (file: string) => {
    const text = files.get(file) ?? onDisk(file);
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
    skipLibCheck: true,
    types: [],
    // ArkTS modules are bare specifiers (`import { window } from '@kit.ArkUI'`,
    // `import http from '@ohos.net.http'`). Without this map the resolver cannot find
    // them, every such import degrades to `any`, and hover answers
    // "window.WindowStage = /*unresolved*/ any" — measured on EntryAbility.ets.
    baseUrl: etsRoot,
    // ArkTS specifiers are dotted (`@ohos.window` → `api/@ohos.window.d.ts`), not
    // slashed: a `@ohos/*` mapping resolves the kit file but not what it re-exports,
    // which leaves `window` as `any` and every `window.X` unresolved (measured).
    paths: {
      "@ohos.*": ["api/@ohos.*"],
      "@kit.*": ["kits/@kit.*"],
      "@system.*": ["api/@system.*"],
      "@arkts.*": ["arkts/@arkts.*"],
    },
  }),
  getDefaultLibFileName: (options: any) =>
    `${etsRoot}/build-tools/ets-loader/node_modules/typescript/lib/${ts.getDefaultLibFileName(options)}`,
  // The map holds the SDK declarations and project sources; the standard library and
  // the compiler's own lib files are only on disk. Without a fallback the program
  // loads no lib and reports "Cannot find name 'JSON'" on clean code — but an
  // unrestricted fallback lets module resolution wander into the SDK's node_modules
  // and exhausts the heap (measured: 4 GB, OOM abort), so it is scoped.
  fileExists: (file: string) => files.has(file) || (onDisk(file) !== undefined),
  readFile: (file: string) => files.get(file) ?? onDisk(file),
  // The resolver probes `.ts`, `.tsx`, `.d.ts`; the SDK also ships `.d.ets`.
  directoryExists: (dir: string) => declarationDirs(sdkRoot).some((d) => dir.startsWith(d)),
  getDirectories: () => ["api", "kits", "component", "arkts"].map((d) => `${etsRoot}/${d}`),
  getScriptKind: (file: string) => (file.endsWith(".ets") ? (ts.ScriptKind.ETS ?? 8) : undefined),
};

const service = ts.createLanguageService(host, ts.createDocumentRegistry());

// Parsed on first use: ~120 declaration files, and only chain completion needs it.
let attributeIndex: AttributeIndex | undefined;
function attributes(): AttributeIndex {
  attributeIndex ??= buildAttributeIndex(ts, files);
  return attributeIndex;
}

let typeIndex: TypeIndex | undefined;
function types(): TypeIndex {
  typeIndex ??= buildTypeIndex(ts, files);
  return typeIndex;
}

let enumIndex: Map<string, string[]> | undefined;
function enums(): Map<string, string[]> {
  enumIndex ??= buildEnumIndex(ts, files);
  return enumIndex;
}

const KEYWORDS = [
  "const", "let", "var", "if", "else", "for", "while", "return", "new", "import", "export",
  "class", "struct", "async", "await", "switch", "case", "break", "continue", "try", "catch",
  "finally", "throw", "interface", "type", "enum", "extends", "implements", "private",
  "public", "protected", "readonly", "static", "this", "null", "true", "false", "undefined",
];

/** Keywords matching the typed prefix; the service offers none, so they are added here. */
function keywordCompletions(lineText: string, character: number): CompletionItem[] {
  const prefix = /([A-Za-z_$][\w$]*)$/.exec(lineText.slice(0, character))?.[1];
  if (!prefix) return [];
  return KEYWORDS.filter((keyword) => keyword.startsWith(prefix) && keyword !== prefix).map((keyword) => ({
    label: keyword,
    kind: CompletionItemKind.Keyword,
    detail: "keyword",
    // Tier 0.5: after reachable names, before the SDK globals.
    sortText: "05" + keyword,
  }));
}

/** Component of the chain the cursor is in, if any. */
function chainComponentAt(file: string, position: { line: number; character: number }): string | undefined {
  return chainComponent((files.get(file) ?? "").split(/\r?\n/), position.line, position.character);
}

/** The whole chain up to the cursor, so a call may span lines. */
function chainText(path: string, line: number, character: number): string {
  const lines = (files.get(path) ?? "").split(/\r?\n/);
  const parts: string[] = [(lines[line] ?? "").slice(0, character)];
  let current = line;
  while (current > 0) {
    const previous = lines[current - 1] ?? "";
    const trimmed = previous.trim();
    if (!(trimmed.startsWith(".") || /[),]$/.test(trimmed) || parts[0].trim().startsWith("."))) break;
    parts.unshift(previous);
    current--;
  }
  return parts.join("");
}

/**
 * Signature declared for an attribute method.
 *
 * The component on the chain is preferred: different components declare different
 * overloads of the same name, and taking "any" signature produced the wrong parameter
 * type (measured on `fontWeight`).
 */
function attributeSignature(member: string, component?: string): string | undefined {
  const index = attributes();
  const own = component ? index.components.get(component) : undefined;
  const fromComponent = own ? index.attributes.get(own)?.get(member) : undefined;
  if (fromComponent) return fromComponent;
  for (const members of index.attributes.values()) {
    const signature = members.get(member);
    if (signature) return signature;
  }
  return undefined;
}
const positionOf = (file: string, offset: number) => {
  const text = originals.get(file) ?? files.get(file) ?? "";
  const source = ts.createSourceFile(file, text, ts.ScriptTarget.ES2021);
  return source.getLineAndCharacterOfPosition(offset);
};
const offsetOf = (file: string, line: number, character: number) => {
  const text = originals.get(file) ?? files.get(file) ?? "";
  const source = ts.createSourceFile(file, text, ts.ScriptTarget.ES2021);
  return source.getPositionOfLineAndCharacter(line, character);
};

/** Original offset → offset inside the transformed text the service sees. */
const serviceOffset = (file: string, offset: number): number => {
  const result = transformed.get(file);
  return result ? toTransformed(result, offset) : offset;
};

/** Transformed offset → original offset (undefined for injected syntax). */
const originalOffset = (file: string, offset: number): number | undefined => {
  const result = transformed.get(file);
  return result ? toOriginal(result, offset) : offset;
};

/** LSP position → offset in the transformed text. */
const servicePosition = (file: string, line: number, character: number): number =>
  serviceOffset(file, offsetOf(file, line, character));

// Explicit streams: the transport never comes from CLI flags, so the server can
// take its own --sdk/--root options.
interface HvoltSettings {
  binaryPath?: string;
  /** Where to record protocol traffic (also settable with --log-file / HVOLT_LSP_LOG). */
  logFile?: string;
  lintOnSave?: boolean;
  /** Refresh diagnostics shortly after typing stops (default: on). */
  lintOnType?: boolean;
  /** Rule ids to drop from codelinter reports, e.g. "@typescript-eslint/no-magic-numbers". */
  lintIgnoreRules?: string[];
  /** Publish type diagnostics from the SDK's TypeScript service (default: on). */
  typeDiagnostics?: boolean;
  /** Debounce for lintOnType, milliseconds (default 1200). */
  lintDelay?: number;
  /** Run codelinter with its broad rule sets (@performance/all + @typescript-eslint/all). */
  lintAllRules?: boolean;
}
let settings: HvoltSettings = {};
let logFile: string | undefined;
const binaryPath = () => settings.binaryPath ?? process.env.HVOLT_BIN ?? "hvolt";

const connection = createConnection(ProposedFeatures.all, process.stdin, process.stdout);

// `--log-file <path>` (or HVOLT_LSP_LOG) records the protocol traffic: which documents
// the editor opened and which requests it made. Useful when a client misbehaves and
// there is no other way to see the exchange.
logFile = option("--log-file") ?? process.env.HVOLT_LSP_LOG;
function log(line: string): void {
  if (!logFile) return;
  try {
    appendFileSync(logFile, `${new Date().toISOString()} ${line}\n`);
  } catch {
    // a broken log file must not break the server
  }
}
const documents = new TextDocuments(TextDocument);

connection.onDidChangeConfiguration((change) => {
  const next = (change?.settings?.hvolt ?? {}) as HvoltSettings;
  if (Object.keys(next).length === 0) return;
  settings = { ...settings, ...next };
  log(`configuration: ${JSON.stringify(settings)}`);
  // Findings from the previous rule set must go, or the editor keeps showing them.
  bySource.delete("lint");
  lintSoon();
  scheduleTypeDiagnostics();
});

connection.onInitialize((params) => {
  log(`initialize rootUri=${params.rootUri ?? "(none)"} client=${params.clientInfo?.name ?? "?"}`);
  settings = (params.initializationOptions?.hvolt ?? {}) as HvoltSettings;
  logFile = settings.logFile ?? logFile;
  return {
    serverInfo: { name: "hvolt-lsp" },
    capabilities: {
      textDocumentSync: TextDocumentSyncKind.Full,
      hoverProvider: true,
      signatureHelpProvider: { triggerCharacters: ["(", ","], retriggerCharacters: [","] },
      completionProvider: { resolveProvider: false, triggerCharacters: [".", "@", "'", "/"] },
      definitionProvider: true,
      renameProvider: true,
      // The editor contributes its own `hvolt.*` commands, so the server must not
      // declare the same ids — vscode-languageclient registers provided commands and
      // throws on a duplicate. One generic entry point keeps both sides happy.
      executeCommandProvider: { commands: ["hvolt.buildDiagnostics", "hvolt.executeCli"] },
    },
  };
});

documents.onDidOpen((e) => {
  log(`didOpen ${e.document.uri} (${e.document.languageId}, ${e.document.getText().split("\n").length} lines)`);
  const path = e.document.uri.replace(/^file:\/\//, "");
  originals.set(path, e.document.getText());
  versions.set(path, e.document.version);
  refreshTransform(path);
});
documents.onDidChangeContent((e) => {
  const path = e.document.uri.replace(/^file:\/\//, "");
  originals.set(path, e.document.getText());
  refreshTransform(path);
  versions.set(path, e.document.version);
});

connection.onHover(({ textDocument, position }) => {
  log(`hover ${textDocument.uri}:${position.line + 1}:${position.character + 1}`);
  const file = textDocument.uri.replace(/^file:\/\//, "");
  const info = service.getQuickInfoAtPosition(file, servicePosition(file, position.line, position.character));
  if (!info) return null;
  let text = ts.displayPartsToString(info.displayParts ?? []);
  const docs = ts.displayPartsToString(info.documentation ?? []);
  if (!text.trim() || text.trim() === "any") {
    // Inside a component chain the fork cannot resolve anything, so hover answered
    // `any` for `.fontWeight(…)` itself. The declaration knows the signature.
    const lines = (files.get(file) ?? "").split(/\r?\n/);
    const lineText = lines[position.line] ?? "";
    const word = /[\w$]+/g;
    let under = "";
    for (let match = word.exec(lineText); match; match = word.exec(lineText)) {
      if (position.character >= match.index && position.character <= match.index + match[0].length) {
        under = match[0];
        break;
      }
    }
    const signature = under ? attributeSignature(under, chainComponentAt(file, position)) : undefined;
    if (signature) {
      log(`hover fallback → declaration for ${under}`);
      return {
        contents: { kind: MarkupKind.Markdown, value: "```ts\n" + signature.replace(/:\s*T$/, `: ${chainComponentAt(file, position) ?? "T"}`) + "\n```" },
      };
    }
  }
  if (!text.trim()) {
    // Some symbols report documentation but no display parts (measured on
    // `hilog.error`: hover showed only "Prints ERROR logs."). Fall back to the
    // signature the service already knows how to build.
    const items = service.getSignatureHelpItems(file, servicePosition(file, position.line, position.character), {});
    const first = items?.items?.[0];
    if (first) {
      const params = first.parameters.map((parameter: any) => ts.displayPartsToString(parameter.displayParts ?? []));
      text =
        ts.displayPartsToString(first.prefixDisplayParts ?? []) +
        params.join(ts.displayPartsToString(first.separatorDisplayParts ?? [])) +
        ts.displayPartsToString(first.suffixDisplayParts ?? []);
    }
  }
  return {
    contents: { kind: MarkupKind.Markdown, value: "```ts\n" + text + "\n```" + (docs ? "\n\n" + docs : "") },
    range: undefined,
  };
});

connection.onSignatureHelp(({ textDocument, position }) => {
  log(`signatureHelp ${textDocument.uri}:${position.line + 1}:${position.character + 1}`);
  const file = textDocument.uri.replace(/^file:\/\//, "");
  const items = service.getSignatureHelpItems(file, servicePosition(file, position.line, position.character), {});
  if (!items) {
    // Component chains are beyond the fork's parser; the declaration still knows the
    // signature, and that is what the user needs while typing the arguments.
    const call = chainCallAt(chainText(file, position.line, position.character));
    const signature = call ? attributeSignature(call.member, chainComponentAt(file, position)) : undefined;
    if (!call || !signature) return null;
    const parameters = parameterTypes(signature).map((text) => ({ label: text }));
    return {
      signatures: [{ label: signature.replace(/\):\s*T$/, "): T"), parameters }],
      activeSignature: 0,
      activeParameter: parameters.length > 0 ? Math.min(call.argument, parameters.length - 1) : 0,
    };
  }
  // The SDK's functions sometimes come back with empty display parts (measured on
  // `hilog.info`), so the label falls back to the type the service reports for the
  // same symbol — an empty signature box is worse than no signature help.
  // The callee is to the left of the cursor (inside the argument list), so quick info
  // has to be asked for at the call's name, not at the cursor.
  const quickLabel = (): string => {
    // The callee sits to the left of the cursor: ask quick info for the name, not for
    // the argument position.
    const text = originals.get(file) ?? "";
    const lineText = text.split(/\r?\n/)[position.line] ?? "";
    const cursor = Math.min(position.character, lineText.length);
    const open = lineText.lastIndexOf("(", cursor);
    if (open < 0) return "";
    let start = open;
    while (start > 0 && /[\w$]/.test(lineText[start - 1])) start--;
    const info = service.getQuickInfoAtPosition(file, serviceOffset(file, offsetOf(file, position.line, start)));
    return ts.displayPartsToString(info?.displayParts ?? []).trim();
  };
  const signatures = items.items.map((item: any) => {
    const params = item.parameters.map((parameter: any) => ts.displayPartsToString(parameter.displayParts ?? []));
    const prefix = ts.displayPartsToString(item.prefixDisplayParts ?? []);
    const separator = ts.displayPartsToString(item.separatorDisplayParts ?? []);
    const suffix = ts.displayPartsToString(item.suffixDisplayParts ?? []);
    const docs = ts.displayPartsToString(item.documentation ?? []);
    const assembled = prefix + params.join(separator) + suffix;
    return {
      label: assembled.trim() || quickLabel() || "(unknown signature)",
      documentation: docs ? { kind: MarkupKind.Markdown, value: docs } : undefined,
      parameters: params.map((text: string) => ({ label: text })),
    };
  });
  if (signatures.length === 0) return null;
  return {
    signatures,
    activeSignature: Math.min(items.selectedItemIndex ?? 0, signatures.length - 1),
    activeParameter:
      signatures[0] && signatures[0].parameters.length > 0
        ? Math.min(items.argumentIndex ?? 0, signatures[0].parameters.length - 1)
        : 0,
  };
});

connection.onCompletion(({ textDocument, position }) => {
  log(`completion ${textDocument.uri}:${position.line + 1}:${position.character + 1}`);
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
  // `Alignment.|` — component attributes take enums, and the fork answers nothing here
  // because the chain does not parse. Members come from the declarations.
  const enumRef = enumAt(lineText.slice(0, position.character));
  const enumName = enumRef && enums().has(enumRef.name) ? enumRef.name : undefined;
  if (enumName) {
    const members = enums().get(enumName) ?? [];
    log(`enum completion ${enumName} → ${members.length}`);
    return members.map((name) => ({
      label: name,
      kind: CompletionItemKind.EnumMember,
      detail: enumName,
    }));
  }

  // Inside a chain call (`alignRules({ | })`) the argument is an object literal whose
  // shape is declared in the SDK; the fork cannot parse the chain at all, so the
  // members are served from the declarations.
  const call = chainCallAt(chainText(file, position.line, position.character));
  if (call) {
    const signature = attributeSignature(call.member, chainComponentAt(file, position));
    const declared = signature ? parameterTypes(signature) : [];
    const target = declared[call.argument] ?? declared[0];

    // The parameter may be an enum (`fontWeight(value: FontWeight)`); the name itself is
    // what the user types next, and in the fork's 1,755-entry global list it is buried,
    // so the matching enum names are offered first.
    if (call.keys.length === 0) {
      const wanted = (target ?? "").split("|").map((part) => part.trim());
      const known = wanted.filter((name) => enums().has(name));
      const typed = /([A-Za-z_$][\w$]*)$/.exec(lineText.slice(0, position.character))?.[1] ?? "";
      if (known.length > 0 && (!typed || known.some((name) => name.startsWith(typed)))) {
        log(`enum name completion ${call.member}(${call.argument}) → ${known.join(" | ")}`);
        return known.map((name) => ({
          label: name,
          kind: CompletionItemKind.Enum,
          detail: `expected by ${call.member}()`,
          sortText: "0",
        }));
      }
    }

    const members = membersOf(types(), target, call.keys);
    if (members.size > 0) {
      log(`argument completion ${call.member}(${call.argument})${call.keys.map((k) => `.${k}`).join("")} → ${members.size}`);
      return [...members].map(([name, declared]) => ({
        label: name,
        kind: CompletionItemKind.Property,
        detail: declared,
      }));
    }
  }

  // A component chain is beyond the fork's parser, so attributes come from the
  // declarations instead of from the language service.
  const chain = chainComponent((files.get(file) ?? "").split(/\r?\n/), position.line, position.character);
  // Only after a closed call: inside `(` the answer is argument completion, and
  // offering the component's 268 attributes there was the reported bug.
  if (chain && !call && /\.[\w$]*$/.test(lineText.slice(0, position.character))) {
    const index = attributes();
    const attributeName = index.components.get(chain);
    const members = attributeName ? index.attributes.get(attributeName) : undefined;
    if (members && members.size > 0) {
      // Offer only what the typed prefix can still match: the full 268-entry list
      // buries the attributes the user is after.
      const prefix = /\.([\w$]*)$/.exec(lineText.slice(0, position.character))?.[1] ?? "";
      const entries = [...members].filter(([name]) => name.startsWith(prefix));
      log(`chain completion ${chain}.${prefix || "*"} → ${entries.length}/${members.size} attributes`);
      return entries.map(([name, signature]) => ({
        label: name,
        kind: CompletionItemKind.Method,
        // Inherited signatures end in the generic `: T`; name the attribute instead.
        detail: signature.replace(/\):\s*T$/, `): ${attributeName}`),
      }));
    }
  }

  const completions = service.getCompletionsAtPosition(file, servicePosition(file, position.line, position.character), {});
  if (!completions) return keywordCompletions(lineText, position.character);
  // The service returns 1,757 entries in its own order, which puts `arguments` first and
  // buries the local variable being typed. Locals, parameters, imports and members are
  // ranked ahead of everything else.
  // Tier 0: what is reachable in this scope (locals, parameters, imports, members of
  // the current object). Tier 1: keywords. Tier 2: global declarations. Tier 3: the
  // rest — the SDK contributes ~1,750 entries at tier 2+.
  const rank = (kind: string): string => {
    switch (kind) {
      case ts.ScriptElementKind.localVariableElement:
      case ts.ScriptElementKind.parameterElement:
      // `let counter = 1` reports kind "let" — measured; missing it ranked the local
      // below every SDK global.
      case "let":
      case ts.ScriptElementKind.variableElement:
      case ts.ScriptElementKind.constElement:
      case ts.ScriptElementKind.alias:
      case ts.ScriptElementKind.moduleElement:
        return "0";
      case ts.ScriptElementKind.memberVariableElement:
      case ts.ScriptElementKind.memberGetAccessorElement:
      case ts.ScriptElementKind.memberSetAccessorElement:
      case ts.ScriptElementKind.memberFunctionElement:
        return "0";
      case ts.ScriptElementKind.functionElement:
      case ts.ScriptElementKind.classElement:
      case ts.ScriptElementKind.enumElement:
        return "2";
      default:
        return "9";
    }
  };
  // Which names this file actually uses: libraries contribute `variableElement`s too, so
  // the kind alone cannot tell a local from a lib global (measured: `Concurrent` came
  // before the local `counter`). Anything spelled in the file ranks above the SDK.
  const usedHere = new Set((files.get(file) ?? "").match(/[A-Za-z_$][\w$]*/g) ?? []);
  const items = completions.entries.map((entry: any) => ({
    label: entry.name,
    kind: entry.kind === ts.ScriptElementKind.memberFunctionElement ? CompletionItemKind.Method : CompletionItemKind.Text,
    detail: entry.kind,
    sortText: (usedHere.has(entry.name) ? rank(entry.kind) : "2") + (entry.sortText ?? entry.name),
  }));
  // Keywords are not in the service's list at all; without them `con` never offers `const`.
  const merged = [...keywordCompletions(lineText, position.character), ...items];
  // Sorted here as well: the server's order is what a plain client shows first.
  merged.sort((a, b) => (a.sortText ?? a.label).localeCompare(b.sortText ?? b.label));
  return merged;
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
  const defs = service.getDefinitionAtPosition(file, servicePosition(file, position.line, position.character)) ?? [];
  return defs.map((def: any) => {
    const start = positionOf(def.fileName, def.textSpan.start);
    const end = positionOf(def.fileName, def.textSpan.start + def.textSpan.length);
    return {
      uri: "file://" + def.fileName,
      range: { start: { line: start.line, character: start.character }, end: { line: end.line, character: end.character } },
    };
  });
});

connection.onRenameRequest(({ textDocument, position, newName }) => {
  const file = textDocument.uri.replace(/^file:\/\//, "");
  const rename = service.findRenameLocations(file, servicePosition(file, position.line, position.character), false, false) ?? [];
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
        range: (() => {
          const r = rangeOf(diagnostic, files.get(diagnostic.path));
          return { start: { line: r.line, character: r.start }, end: { line: r.line, character: r.end } };
        })(),
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

interface RunResult {
  code: number | null;
  output: string;
}

function run(binary: string, args: string[], cwd: string): Promise<RunResult> {
  return new Promise((resolve) => {
    // Both streams matter: hvigor writes progress to stdout and the diagnostics to
    // stderr, so the two are concatenated rather than picking one. The exit code is
    // the only reliable success signal — grepping the text misses messages such as
    // "SDK not available".
    execFile(binary, args, { cwd, maxBuffer: 64 * 1024 * 1024 }, (error, stdout, stderr) => {
      const raw = (error as { code?: unknown } | null)?.code;
      resolve({ code: typeof raw === "number" ? raw : error ? 1 : 0, output: `${stdout ?? ""}${stderr ?? ""}` });
    });
  });
}

let lintTimer: NodeJS.Timeout | undefined;
let lintGeneration = 0;
let lintRunning = false;
let lintAgain = false;

/**
 * Lint after a short pause.
 *
 * Measured (Linux, 3-file project): a full run takes 2.2 s and re-translates the
 * tree every time, so it always reflects the current text. Path-scoped runs are not
 * usable — codelinter resolves paths inside that translated tree, where the layout
 * differs, and answers "engine failed". A run that started before the latest
 * keystroke is therefore dropped instead of published, and overlapping runs coalesce.
 */
function lintSoon(paths?: string[]): void {
  if (settings.lintOnSave === false && !paths) return;
  clearTimeout(lintTimer);
  lintTimer = setTimeout(() => void lintNow(), paths ? settings.lintDelay ?? 2000 : 400);
}

async function lintNow(paths?: string[]): Promise<void> {
  if (lintRunning) {
    lintAgain = true;
    return;
  }
  lintRunning = true;
  const generation = ++lintGeneration;
  const startedAt = Date.now();
  let overlayFile: string | undefined;
  try {
    // Hand the editor's buffers over, so lint judges what is on screen rather than
    // the last save — otherwise the indicator is silent exactly when it matters.
    const overlay = new Map<string, string>();
    for (const document of documents.all()) {
      const path = document.uri.replace(/^file:\/\//, "");
      if (!path.endsWith(".ets")) continue;
      const onDisk = existsSync(path) ? readFileSync(path, "utf8") : "";
      if (onDisk === document.getText()) continue;
      overlay.set(path, document.getText());
    }
    const args = ["lint", "--json"];
    if (settings.lintAllRules) args.push("--all-rules");
    if (overlay.size > 0) {
      overlayFile = join(tmpdir(), `hvolt-lsp-overlay-${process.pid}-${generation}.json`);
      writeFileSync(overlayFile, JSON.stringify(Object.fromEntries(overlay)));
      args.push("--overlay-file", overlayFile);
    }
    const { output } = await run(binaryPath(), args, root);
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
    const ignored = new Set(settings.lintIgnoreRules ?? []);
    if (ignored.size > 0) {
      const defects = ((report as { defects?: Array<{ rule?: string }> }).defects ?? []).filter(
        (defect) => !(defect.rule && ignored.has(defect.rule)),
      );
      (report as { defects?: unknown }).defects = defects;
    }
    if (!report) {
      if (process.env.HVOLT_LSP_DEBUG) process.stderr.write("hvolt-lsp: lint report not found\n");
      return;
    }
    if (generation !== lintGeneration) return; // a newer run supersedes this one
    const parsed = fromLint(report, root);
    log(`lint → ${parsed.length} diagnostics in ${Date.now() - startedAt} ms (${parsed.length === 0 ? "clean" : "findings"})`);
    publish("lint", parsed);
  } finally {
    if (overlayFile) {
      try {
        rmSync(overlayFile, { force: true });
      } catch {
        // a leftover temp file is harmless; never mask the lint result
      }
    }
    lintRunning = false;
    if (lintAgain) {
      lintAgain = false;
      void lintNow(paths);
    }
  }
}

/**
 * Type diagnostics from the SDK's own TypeScript service.
 *
 * It understands ArkTS types (that is the same service behind hover), but its parser
 * rejects legal ArkUI block-chains: measured on `examples/hello-world`, `Index.ets`
 * reports TS1005 "'}' expected' style errors at a `}.modifier(…)` chain plus two
 * cascade errors, while `EntryAbility.ets` is clean. A file whose diagnostics include
 * a parse error immediately after a `}.` chain is therefore skipped as a whole rather
 * than published with fabricated red lines; genuine type errors in other files still
 * reach the editor.
 */
let typeTimer: NodeJS.Timeout | undefined;
let typeRunning = false;
let typeQueued = false;

/**
 * Debounced because a type check walks the whole SDK declaration set: doing it per
 * keystroke OOMs the process (measured: node aborts on heap exhaustion). Only open
 * documents are checked, which is also all an editor can show.
 */
function scheduleTypeDiagnostics(): void {
  if (settings.typeDiagnostics === false) return;
  clearTimeout(typeTimer);
  typeTimer = setTimeout(() => void arktsDiagnostics(), 600);
}

function arktsDiagnostics(): void {
  if (settings.typeDiagnostics === false) return;
  if (typeRunning) {
    // Coalesce: one check at a time, one more afterwards if anything changed.
    typeQueued = true;
    return;
  }
  typeRunning = true;
  const collected: Diagnostic[] = [];
  const open = [...documents.all()]
    .map((document) => document.uri.replace(/^file:\/\//, ""))
    .filter((path) => path.endsWith(".ets") && path.startsWith(root));
  for (const path of open) {
    // The block-chain test has to look at the user's text: the transformed copy no
    // longer contains `}.modifier(…)`, which is exactly what marks an ArkUI DSL file.
    const text = originals.get(path) ?? files.get(path) ?? "";
    if (!text) continue;
    // The parser rejects `View() { … }.modifier(…)`, so the test is on the source, not
    // on a diagnostic: any `}` directly followed by `.name(` marks an ArkUI DSL chain.
    const syntactic = service.getSyntacticDiagnostics(path);
    const chained = /\}\s*\.[A-Za-z_$][\w$]*\s*\(/.test(text);
    if (chained) {
      log(`type diagnostics skipped for ${path.replace(root + "/", "")}: ArkUI block-chain parse`);
      continue;
    }
    const all = [...syntactic, ...service.getSemanticDiagnostics(path)];
    for (const d of all) {
      const pos = d.file?.getLineAndCharacterOfPosition(d.start ?? 0);
      if (!pos) continue;
      collected.push({
        path,
        line: pos.line + 1,
        column: pos.character + 1,
        length: Math.max(1, d.length ?? 1),
        message: ts.flattenDiagnosticMessageText(d.messageText, " "),
        severity: d.category === ts.DiagnosticCategory.Error ? "error" : "warning",
        source: "arkts",
      });
    }
  }
  publish("arkts", collected);
  typeRunning = false;
  if (typeQueued) {
    typeQueued = false;
    scheduleTypeDiagnostics();
  }
}

function validateTomlDocument(path: string, text: string): void {
  if (!path.endsWith("hvolt.toml")) return;
  const moduleDir = dirname(path);
  const diagnostics = validateToml(text, {
    // A route may be declared as `pages/Index`, `pages/Index.ets`, or `pages/Index/Index.ets`
    // depending on where it is written; only the file's existence matters here.
    routeExists: (route) =>
      [route, `${route}.ets`, `${route}.ts`, join(route, "Index.ets")].some((candidate) =>
        existsSync(join(moduleDir, candidate)),
      ),
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
documents.onDidOpen(() => scheduleTypeDiagnostics());
documents.onDidChangeContent(() => scheduleTypeDiagnostics());
documents.onDidSave(() => scheduleTypeDiagnostics());
documents.onDidSave(() => lintSoon());
documents.onDidChangeContent((e) => {
  if (settings.lintOnType === false) return;
  const path = e.document.uri.replace(/^file:\/\//, "");
  if (!path.endsWith(".ets")) return;
  const relative = path.startsWith(root + "/") ? path.slice(root.length + 1) : path;
  lintSoon([relative]);
});

connection.onExecuteCommand(async ({ command, arguments: args }) => {
  const cwd = (args?.[0] as string | undefined) ?? root;

  if (command === "hvolt.buildDiagnostics") {
    const { output } = await run(binaryPath(), ["build"], cwd);
    const parsed = parseHvigor(output, root);
    if (process.env.HVOLT_LSP_DEBUG) {
      process.stderr.write(`hvolt-lsp: build in ${cwd} → ${output.length} bytes, ${parsed.length} diagnostics\n`);
    }
    publish("build", parsed);
    return null;
  }

  const cliCommand = command === "hvolt.executeCli" ? String(args?.[1] ?? "") : command;
  const request = (command === "hvolt.executeCli" ? args?.[2] : args?.[1]) as CommandRequest | undefined;
  const spec = commandSpec(cliCommand, request ?? {});
  if (!spec) return { ok: false, message: `unknown command ${cliCommand}` };

  const started = Date.now();
  const { code, output } = await run(binaryPath(), spec.args, cwd);
  const seconds = ((Date.now() - started) / 1000).toFixed(1);

  // A build's failures belong on the file that caused them.
  if (spec.args[0] === "build" || spec.args[0] === "assemble") {
    publish("build", parseHvigor(output, root));
  }
  const failed = code !== 0;
  const summary = `hvolt ${spec.args.join(" ")} → ${failed ? "failed" : "ok"} (${seconds}s)`;
  // A notification, not showInformationMessage(): the latter is a request and dangles
  // until the client answers, which some clients never do.
  connection.sendNotification("window/showMessage", { type: 3, message: summary });
  if (process.env.HVOLT_LSP_DEBUG) process.stderr.write(`hvolt-lsp: ${summary}\n`);
  return { ok: !failed, seconds, tail: output.split(/\r?\n/).slice(-20).join("\n") };
});

documents.listen(connection);
connection.listen();
