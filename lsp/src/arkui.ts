// ArkUI component attributes, taken from the SDK's own declarations.
//
// The SDK's TypeScript service can't answer for a component chain (`Text('x').`, then
// `.width(…)`): its parser rejects the block syntax, so completion there comes back
// empty. The declarations themselves describe the attributes, so they are parsed once
// and served directly — no guessing of names.

import type { TsModule } from "./sdk.js";

export interface AttributeIndex {
  /** `TextAttribute` -> member names, inheritance already resolved. */
  attributes: Map<string, Set<string>>;
  /** `Text` -> `TextAttribute`. */
  components: Map<string, string>;
}

/** Parse `XAttribute` interfaces out of the SDK's component declarations. */
export function buildAttributeIndex(ts: TsModule, declarations: Map<string, string>): AttributeIndex {
  const own = new Map<string, Set<string>>();
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
      const members = new Set<string>();
      for (const member of statement.members) {
        const memberName = member.name && ts.isIdentifier(member.name) ? member.name.text : undefined;
        if (memberName) members.add(memberName);
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
  const resolved = new Map<string, Set<string>>();
  const resolve = (name: string, seen = new Set<string>()): Set<string> => {
    const cached = resolved.get(name);
    if (cached) return cached;
    if (seen.has(name)) return new Set();
    seen.add(name);
    const all = new Set(own.get(name) ?? []);
    for (const parent of parents.get(name) ?? []) {
      for (const member of resolve(parent, seen)) all.add(member);
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
  let current = line;
  let text = (lines[current] ?? "").slice(0, character);
  // Walk up while the line is only a continuation of the chain.
  while (current > 0 && /^\s*\./.test(text)) {
    current--;
    text = (lines[current] ?? "") + text.trimStart();
    if (!/^\s*\./.test((lines[current] ?? ""))) break;
  }
  const match = /([A-Z][A-Za-z0-9_]*)\s*\(/.exec(text);
  return match?.[1];
}
