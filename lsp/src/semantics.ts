// ArkTS-aware semantic tokens.
//
// The SDK's classifier crashes on ArkUI syntax — `getEncodedSemanticClassifications`
// aborts with `Debug Failure. False expression: Classification had non-positive length
// of 0` (measured both with and without the transform, and with the deprecated
// `getSemanticClassifications` as well), while it runs on plain TypeScript files. Rather
// than depend on that, the tokens come from the declarations the server already indexes:
// which names are components, which are enums and enum members, which are attributes,
// plus decorators and the DSL functions.
//
// Only names are emitted: keywords, strings and operators are already coloured by the
// grammar, and a second opinion there fights the theme.

export interface TokenIndexes {
  components: Set<string>;
  structs: Set<string>;
  enums: Map<string, string[]>;
  decorators: Set<string>;
  functions: Set<string>;
  /** Component → attribute names, for `.fontSize(` on a component chain. */
  attributes: (component: string | undefined) => Set<string>;
}

export interface SemanticToken {
  line: number;
  character: number;
  length: number;
  /** Index into the legend the server declares. */
  type: number;
}

/** Order must match the legend in server.ts. */
export const TOKEN_TYPES = [
  "namespace", "class", "interface", "enum", "typeParameter", "type",
  "parameter", "variable", "property", "method", "function", "enumMember",
  "decorator", "keyword", "comment", "string", "number", "operator",
] as const;

const typeIndex = (name: (typeof TOKEN_TYPES)[number]): number => TOKEN_TYPES.indexOf(name);

const IDENTIFIER = /[A-Za-z_$][\w$]*/g;

interface ScanState {
  blockComment: boolean;
  template: boolean;
}

/**
 * Blank out everything that is not code, keeping the string length so positions stay
 * exact. Measured failure without this: `import { hilog } from '@kit.PerformanceAnalysisKit'`
 * produced a `decorator` token for `kit` (the `@` before it) and a `property` token for
 * `PerformanceAnalysisKit` (the `.`), so a module specifier was coloured as code.
 */
function maskNonCode(line: string, state: ScanState): string {
  const out = line.split("");
  let index = 0;
  while (index < line.length) {
    if (state.blockComment) {
      const end = line.indexOf("*/", index);
      const stop = end === -1 ? line.length : end + 2;
      for (let k = index; k < stop; k++) out[k] = " ";
      index = stop;
      if (end !== -1) state.blockComment = false;
      continue;
    }
    if (state.template) {
      while (index < line.length) {
        if (line[index] === "\\") {
          out[index] = " ";
          if (index + 1 < line.length) out[index + 1] = " ";
          index += 2;
          continue;
        }
        const closing = line[index] === "`";
        out[index] = " ";
        index++;
        if (closing) {
          state.template = false;
          break;
        }
      }
      continue;
    }
    const character = line[index];
    if (character === "/" && line[index + 1] === "/") {
      for (let k = index; k < line.length; k++) out[k] = " ";
      break;
    }
    if (character === "/" && line[index + 1] === "*") {
      state.blockComment = true;
      out[index] = " ";
      out[index + 1] = " ";
      index += 2;
      continue;
    }
    if (character === '"' || character === "'") {
      const quote = character;
      out[index] = " ";
      index++;
      while (index < line.length) {
        if (line[index] === "\\") {
          out[index] = " ";
          if (index + 1 < line.length) out[index + 1] = " ";
          index += 2;
          continue;
        }
        const closing = line[index] === quote;
        out[index] = " ";
        index++;
        if (closing) break;
      }
      continue;
    }
    if (character === "`") {
      state.template = true;
      out[index] = " ";
      index++;
      continue;
    }
    index++;
  }
  return out.join("");
}

/** The name right before `at`, skipping whitespace. */
function previousWord(line: string, at: number): { word: string; end: number } {
  let index = at - 1;
  while (index >= 0 && /\s/.test(line[index])) index--;
  const end = index + 1;
  while (index >= 0 && /[\w$]/.test(line[index])) index--;
  return { word: line.slice(index + 1, end), end };
}

/**
 * Tokens for one file. `chainComponent` answers "which component does the chain at this
 * line belong to", because an attribute's name means nothing without it.
 */
export function semanticTokens(
  text: string,
  indexes: TokenIndexes,
  chainComponent: (line: number) => string | undefined,
): SemanticToken[] {
  const lines = text.split(/\r?\n/);
  const tokens: SemanticToken[] = [];
  const state: ScanState = { blockComment: false, template: false };
  for (let line = 0; line < lines.length; line++) {
    const content = lines[line];
    // Only code is classified: strings, template literals and comments are the grammar's
    // business, and a module specifier is not a name.
    const code = maskNonCode(content, state);
    for (const match of code.matchAll(IDENTIFIER)) {
      const word = match[0];
      const start = match.index ?? 0;
      const before = content.slice(0, start).trimEnd();
      let type: number | undefined;

      if (before.endsWith("@")) {
        type = typeIndex("decorator");
      } else if (indexes.components.has(word) || indexes.structs.has(word)) {
        type = typeIndex("class");
      } else if (indexes.enums.has(word)) {
        type = typeIndex("enum");
      } else if (indexes.decorators.has(word)) {
        type = typeIndex("decorator");
      } else if (indexes.functions.has(word)) {
        type = typeIndex("function");
      } else if (
        content.slice(start + word.length).trimStart().startsWith(":") &&
        !before.endsWith("?")
      ) {
        // `message: string = …` and `{ align: X }` are properties; so is an annotated
        // parameter, which is fine — VS Code colours both as a property.
        type = typeIndex("property");
      } else if (before.trim() === "" && content.slice(start + word.length).trimStart().startsWith("(")) {
        // A name at the start of a line followed by a call is a member declaration
        // (`build() {`). Calls elsewhere are the grammar's business.
        type = typeIndex("method");
      } else if (before.endsWith(".")) {
        const receiver = previousWord(content, before.length - 1).word;
        if (indexes.enums.has(receiver) && (indexes.enums.get(receiver) ?? []).includes(word)) {
          type = typeIndex("enumMember");
        } else if (chainComponent(line) && indexes.attributes(chainComponent(line)).has(word)) {
          type = typeIndex("method");
        } else {
          type = typeIndex("property");
        }
      }
      if (type === undefined) continue;
      tokens.push({ line, character: start, length: word.length, type });
    }
  }
  return tokens;
}

/** Encode tokens as the protocol's delta pairs. */
export function encode(tokens: SemanticToken[]): number[] {
  const data: number[] = [];
  let previousLine = 0;
  let previousCharacter = 0;
  for (const token of tokens) {
    const deltaLine = token.line - previousLine;
    const deltaCharacter = deltaLine === 0 ? token.character - previousCharacter : token.character;
    data.push(deltaLine, deltaCharacter, token.length, token.type, 0);
    previousLine = token.line;
    previousCharacter = token.character;
  }
  return data;
}
