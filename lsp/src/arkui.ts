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
