// ArkUI component attributes, taken from the SDK's own declarations.
//
// The SDK's TypeScript service can't answer for a component chain (`Text('x').`, then
// `.width(…)`): its parser rejects the block syntax, so completion there comes back
// empty. The declarations themselves describe the attributes, so they are parsed once
// and served directly — no guessing of names.

import type { TsModule } from "./sdk.js";

export interface AttributeIndex {
  /** `TextAttribute` -> member name → signature text, inheritance already resolved. */
  attributes: Map<string, Map<string, string>>;
  /** `Text` -> `TextAttribute`. */
  components: Map<string, string>;
}

/** Parse `XAttribute` interfaces out of the SDK's component declarations. */
export function buildAttributeIndex(ts: TsModule, declarations: Map<string, string>): AttributeIndex {
  const own = new Map<string, Map<string, string>>();
  const parents = new Map<string, string[]>();
  const components = new Map<string, string>();

  for (const [path, text] of declarations) {
    if (!/[\\/]component[\\/]/.test(path)) continue;
    const source = ts.createSourceFile(path, text, ts.ScriptTarget.ES2021, true);
    for (const statement of source.statements) {
      // Attributes are declared as classes (`declare class ButtonAttribute extends
      // CommonMethod<ButtonAttribute>`) and the component factories as interfaces
      // (`interface TextInterface { (…): TextAttribute }`); both matter.
      if (!ts.isInterfaceDeclaration(statement) && !ts.isClassDeclaration(statement)) continue;
      if (!statement.name) continue;
      const name = statement.name.text;
      const members = new Map<string, string>();
      for (const member of statement.members) {
        const memberName = member.name && ts.isIdentifier(member.name) ? member.name.text : undefined;
        if (!memberName) continue;
        // Keep the declaration text so completion can show `align(value: Alignment): TextAttribute`
        // instead of a bare name.
        const signature = member.getText
          ? member
              .getText()
              .split("\n")[0]
              .replace(/\s*\{\s*$/, "")
              .replace(/;$/, "")
              .trim()
          : memberName;
        members.set(memberName, signature);
      }
      own.set(name, members);
      parents.set(
        name,
        (statement.heritageClauses ?? []).flatMap((clause: any) =>
          clause.types.map((type: any) => (type.expression?.text as string) ?? "").filter(Boolean),
        ),
      );
      // `interface TextInterface { (content?: string): TextAttribute }` names the pair.
      if (name.endsWith("Interface")) {
        components.set(name.slice(0, -"Interface".length), `${name.slice(0, -"Interface".length)}Attribute`);
      }
    }
  }

  // Resolve inheritance: an attribute object exposes its own members plus its ancestors'.
  const resolved = new Map<string, Map<string, string>>();
  const resolve = (name: string, seen = new Set<string>()): Map<string, string> => {
    const cached = resolved.get(name);
    if (cached) return cached;
    if (seen.has(name)) return new Map();
    seen.add(name);
    const all = new Map(own.get(name) ?? []);
    for (const parent of parents.get(name) ?? []) {
      for (const [member, signature] of resolve(parent, seen)) all.set(member, signature);
    }
    resolved.set(name, all);
    return all;
  };
  for (const name of own.keys()) resolve(name);

  return { attributes: resolved, components };
}

/**
 * Component name for a chain that ends at the cursor.
 *
 * Covers both layouts in real ArkTS: `Text('x').wi` on one line, and a component call
 * with the chain on following lines that start with `.`.
 */
export function chainComponent(lines: string[], line: number, character: number): string | undefined {
  // Assemble the whole chain first: the component call may be several lines above
  // (measured: `Text(this.message)` then `.id(…)`, `.fontSize(…)`, `.align`), and
  // taking only the line above yields a modifier call instead of the component.
  const parts: string[] = [(lines[line] ?? "").slice(0, character)];
  let current = line;
  while (current > 0) {
    const previous = lines[current - 1] ?? "";
    const trimmed = previous.trim();
    const continues = trimmed.startsWith(".") || /[),]$/.test(trimmed) || parts[0].trim().startsWith(".");
    if (!continues) break;
    parts.unshift(previous);
    current--;
  }
  const text = parts.join("");
  // The component is the first capitalised call that is not a `.member(` access.
  for (const match of text.matchAll(/(^|[^.\w])([A-Z][A-Za-z0-9_]*)\s*\(/g)) {
    return match[2];
  }
  return undefined;
}

/** Declared type → property name → declared type, for the whole component tree. */
export type TypeIndex = Map<string, Map<string, string>>;

/** Collect property members of every interface/class in the declarations. */
export function buildTypeIndex(ts: TsModule, declarations: Map<string, string>): TypeIndex {
  const types: TypeIndex = new Map();
  for (const [path, text] of declarations) {
    if (!/[\\/]component[\\/]/.test(path)) continue;
    const source = ts.createSourceFile(path, text, ts.ScriptTarget.ES2021, true);
    for (const statement of source.statements) {
      if (!ts.isInterfaceDeclaration(statement) && !ts.isClassDeclaration(statement)) continue;
      const name = statement.name?.text;
      if (!name) continue;
      const members = types.get(name) ?? new Map<string, string>();
      for (const member of statement.members) {
        if (!member.name || !ts.isIdentifier(member.name)) continue;
        const declared = member.type ? member.type.getText() : "unknown";
        members.set(member.name.text, declared.replace(/^\?\s*/, ""));
      }
      for (const clause of statement.heritageClauses ?? []) {
        for (const type of clause.types) {
          const parent = type.expression?.text;
          if (!parent) continue;
          for (const [member, declared] of types.get(parent) ?? []) {
            if (!members.has(member)) members.set(member, declared);
          }
        }
      }
      types.set(name, members);
    }
  }
  return types;
}

export interface ChainCall {
  /** Attribute method being called, e.g. `alignRules`. */
  member: string;
  /** 0-based argument index at the cursor. */
  argument: number;
  /** Property names enclosing the cursor, outermost first (`["center"]` for `center: { | }`). */
  keys: string[];
}

/**
 * Where the cursor sits inside a component-attribute call.
 *
 * The declarations describe the parameter types, so the object literal can be
 * completed without the language service — which cannot parse these chains at all.
 */
export function chainCallAt(text: string): ChainCall | undefined {
  const open = lastUnclosedParen(text);
  if (open < 0) return undefined;
  const callee = /\.([\w$]+)\s*$/.exec(text.slice(0, open));
  if (!callee) return undefined;
  const inside = text.slice(open + 1);
  const keys: string[] = [];
  let depth = 0;
  let argument = 0;
  let currentKey = "";
  for (let i = 0; i < inside.length; i++) {
    const ch = inside[i];
    if (ch === "{" ) {
      if (currentKey) keys.push(currentKey);
      currentKey = "";
      depth++;
    } else if (ch === "}") {
      depth = Math.max(0, depth - 1);
    } else if (ch === "," && depth === 0) {
      argument++;
    } else if (ch === ":" && depth > 0 && !currentKey) {
      currentKey = "";
    }
    if (depth > 0) {
      const keyMatch = /([\w$]+)\s*:\s*$/.exec(inside.slice(0, i + 1));
      if (keyMatch && /[{,]\s*[\w$]*\s*:/.test(inside.slice(0, i + 1).slice(-40))) {
        currentKey = keyMatch[1];
      }
    }
  }
  return { member: callee[1], argument, keys };
}

function lastUnclosedParen(text: string): number {
  let depth = 0;
  for (let i = text.length - 1; i >= 0; i--) {
    const ch = text[i];
    if (ch === ")") depth++;
    else if (ch === "(") {
      if (depth === 0) return i;
      depth--;
    }
  }
  return -1;
}

/** Parameter types of a declared signature, e.g. `alignRules(alignRule: LocalizedAlignRuleOptions)` → ["LocalizedAlignRuleOptions"]. */
export function parameterTypes(signature: string): string[] {
  const inside = /\(([\s\S]*)\)/.exec(signature);
  if (!inside) return [];
  const parts: string[] = [];
  let depth = 0;
  let current = "";
  for (const ch of inside[1]) {
    if ("<{([".includes(ch)) depth++;
    if (">})]".includes(ch)) depth--;
    if (ch === "," && depth === 0) {
      parts.push(current);
      current = "";
    } else current += ch;
  }
  parts.push(current);
  return parts
    .map((part) => part.split(":").slice(1).join(":").trim())
    .filter(Boolean);
}

/** Members visible after the given property path inside `typeName`. */
export function membersOf(
  types: TypeIndex,
  typeName: string | undefined,
  keys: string[],
): Map<string, string> {
  let current = typeName;
  let members = new Map<string, string>();
  for (const key of keys) {
    const next = current ? types.get(current)?.get(key) : undefined;
    current = next?.replace(/[[\]]/g, "").replace(/^.*\bof\s+/, "").trim();
  }
  if (current) members = types.get(current) ?? new Map();
  return members;
}
