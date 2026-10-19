// The ArkTS→TS transform: the language service can only see scope inside `build()`
// once the ArkUI blocks are rewritten, and every position must still map back.
import assert from "node:assert/strict";
import test from "node:test";
import { toOriginal, toTransformed, transformArkts } from "../dist/transform.mjs";

const COMPONENTS = new Set(["Column", "Text", "Row"]);

test("an ArkUI block becomes a helper call, components stay expressions", () => {
  const source = [
    "build() {",
    "  Column() {",
    "    Text('x').fontSize(20)",
    "  }",
    "  .width('100%')",
    "}",
  ].join("\n");
  const out = transformArkts(source, (name) => COMPONENTS.has(name));
  assert.ok(out.changed);
  assert.match(out.code, /Column\(\)\s*;__arkui\(\(\)=>\{/);
  assert.match(out.code, /Text\('x'\)\.fontSize\(20\)/);
  // The chained modifier attaches to the helper call, so the chain still parses.
  assert.match(out.code, /\}\)\s*\.width\('100%'\)/);
});

test("callbacks and plain blocks are left alone", () => {
  const source = "list.forEach((item) => { if (item) { log(item); } });";
  const out = transformArkts(source, (name) => COMPONENTS.has(name));
  assert.equal(out.changed, false);
  assert.equal(out.code, source);
});

test("positions map both ways, and inserted syntax has no original position", () => {
  const source = ["build() {", "  Column() {", "    let counter = 1;", "  }", "}"].join("\n");
  const out = transformArkts(source, (name) => COMPONENTS.has(name));
  for (const needle of ["build", "counter", "}"]) {
    const orig = source.lastIndexOf(needle);
    const mapped = toTransformed(out, orig);
    assert.equal(toOriginal(out, mapped), orig, `${needle} did not round-trip`);
  }
  const synthetic = out.segments.find((segment) => segment.synthetic);
  assert.ok(synthetic, "no synthetic segment recorded");
  assert.equal(toOriginal(out, synthetic.out), undefined);
});
