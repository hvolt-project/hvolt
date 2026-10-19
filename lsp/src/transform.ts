// ArkTS → TS, for the language service only.
//
// The SDK's parser rejects ArkUI block syntax (`Column() { … }.width(…)`), so inside
// `build()` the language service has no scope at all: completion answers with the 1,757
// global entries and even `coun` never offers the local `counter`. Every other gap
// followed from that one cause.
//
// The transform keeps each ArkUI call as an expression and turns its block into a call
// of a synthetic helper, so the result parses:
//
//   Column() {              Column();__arkui(()=>{
//     Text('x').fontSize(1)     Text('x').fontSize(1)
//   }.width('100%')         }).width('100%')
//
// Positions are mapped, not guessed: copied text keeps its offset, inserted text is
// recorded as synthetic and never maps back to a user position.

export interface Segment {
  /** Start offset in the transformed text. */
  out: number;
  /** Start offset in the original text. */
  orig: number;
  /** Length in the transformed text. */
  length: number;
  /** True when the characters do not exist in the original (inserted syntax). */
  synthetic: boolean;
}

export interface Transformed {
  code: string;
  segments: Segment[];
  /** True when at least one ArkUI block was rewritten. */
  changed: boolean;
}

/** Helper the transformed code calls; declared to the program as an ambient function. */
export const ARKUI_HELPER = "__arkui";

/**
 * Rewrite ArkUI blocks into calls.
 *
 * `isComponent` decides which `Name(...) { … }` sequences are DSL; everything else
 * (callbacks, `if` blocks, object literals) is left untouched.
 */
export function transformArkts(text: string, isComponent: (name: string) => boolean): Transformed {
  const segments: Segment[] = [];
  let out = "";
  let changed = false;

  const copy = (start: number, end: number) => {
    if (end <= start) return;
    // Merge with the previous segment when it is contiguous in both texts.
    const last = segments[segments.length - 1];
    if (last && !last.synthetic && last.out + last.length === out.length && last.orig + last.length === start) {
      last.length += end - start;
    } else {
      segments.push({ out: out.length, orig: start, length: end - start, synthetic: false });
    }
    out += text.slice(start, end);
  };
  const insert = (value: string, anchor: number) => {
    segments.push({ out: out.length, orig: anchor, length: value.length, synthetic: true });
    out += value;
    changed = true;
  };

  let index = 0;
  while (index < text.length) {
    const open = findComponentBlock(text, index, isComponent);
    if (!open) {
      copy(index, text.length);
      break;
    }
    copy(index, open.blockStart);

    // `Name(args) {` → `Name(args);__arkui(()=>{`
    insert(`;${ARKUI_HELPER}(()=>`, open.blockStart);
    insert("{", open.blockStart);

    // Transform the body recursively.
    const closeBrace = matchingBrace(text, open.blockStart);
    if (closeBrace < 0) {
      copy(open.blockStart, text.length);
      break;
    }
    const body = transformArkts(text.slice(open.blockStart + 1, closeBrace), isComponent);
    for (const segment of body.segments) {
      segments.push({
        out: out.length,
        orig: open.blockStart + 1 + segment.orig,
        length: segment.length,
        synthetic: segment.synthetic,
      });
      out += body.code.slice(segment.out, segment.out + segment.length);
    }
    // `})` closes the helper call, so a following `.width(…)` still attaches to it.
    insert("})", closeBrace);
    index = closeBrace + 1;
  }

  return { code: out, segments, changed };
}

/** `Name(args) {` starting at or after `from`, when `Name` is a component. */
function findComponentBlock(
  text: string,
  from: number,
  isComponent: (name: string) => boolean,
): { blockStart: number } | undefined {
  const pattern = /([A-Z][A-Za-z0-9_]*)\s*\(/g;
  pattern.lastIndex = from;
  for (let match = pattern.exec(text); match; match = pattern.exec(text)) {
    if (!isComponent(match[1])) continue;
    const after = skipBalanced(text, match.index + match[0].length - 1);
    if (after < 0) continue;
    const brace = skipSpace(text, after);
    if (text[brace] === "{") return { blockStart: brace };
  }
  return undefined;
}

/** Offset just past the `)` matching the `(` at `open`. */
function skipBalanced(text: string, open: number): number {
  let depth = 0;
  for (let i = open; i < text.length; i++) {
    const ch = text[i];
    if (ch === "(") depth++;
    else if (ch === ")") {
      depth--;
      if (depth === 0) return i + 1;
    } else if (ch === "'" || ch === '"' || ch === "`") {
      const end = text.indexOf(ch, i + 1);
      if (end < 0) return -1;
      i = end;
    }
  }
  return -1;
}

function matchingBrace(text: string, open: number): number {
  let depth = 0;
  for (let i = open; i < text.length; i++) {
    const ch = text[i];
    if (ch === "{") depth++;
    else if (ch === "}") {
      depth--;
      if (depth === 0) return i;
    } else if (ch === "'" || ch === '"' || ch === "`") {
      const end = text.indexOf(ch, i + 1);
      if (end < 0) return -1;
      i = end;
    }
  }
  return -1;
}

function skipSpace(text: string, from: number): number {
  let i = from;
  while (i < text.length && /\s/.test(text[i])) i++;
  return i;
}

/** Original offset → transformed offset. Positions inside copied text map exactly. */
export function toTransformed(transformed: Transformed, orig: number): number {
  for (const segment of transformed.segments) {
    if (segment.synthetic) continue;
    if (orig >= segment.orig && orig <= segment.orig + segment.length) {
      return segment.out + (orig - segment.orig);
    }
  }
  // Past the last copied segment: the tail is copied verbatim.
  return orig;
}

/** Transformed offset → original offset; undefined for inserted syntax. */
export function toOriginal(transformed: Transformed, out: number): number | undefined {
  for (const segment of transformed.segments) {
    if (out >= segment.out && out < segment.out + segment.length) {
      return segment.synthetic ? undefined : segment.orig + (out - segment.out);
    }
  }
  return undefined;
}
