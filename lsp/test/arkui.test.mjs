// ArkUI attribute completion is served from the SDK declarations, because the fork's
// parser rejects component block-chains (`Text('x').` answered 0 candidates).
import assert from "node:assert/strict";
import test from "node:test";
import { createRequire } from "node:module";
import { buildAttributeIndex, chainComponent } from "../dist/arkui.mjs";

const require = createRequire(import.meta.url);
const ts = require("typescript");

const COMMON = `declare class CommonMethod<T> {
  width(value: Length): T;
  height(value: Length): T;
  backgroundColor(value: ResourceColor): T;
}`;
const TEXT = `declare class TextAttribute extends CommonMethod<TextAttribute> {
  fontSize(value: Length): TextAttribute;
  fontWeight(value: FontWeight): TextAttribute;
}`;
const FACTORY = `interface TextInterface { (content?: string): TextAttribute; }`;

const index = buildAttributeIndex(ts, new Map([
  ["/sdk/component/common.d.ts", COMMON],
  ["/sdk/component/text.d.ts", TEXT + "\n" + FACTORY],
]));

test("attributes are collected and inheritance resolved", () => {
  const text = index.attributes.get("TextAttribute");
  assert.ok(text, "TextAttribute missing");
  assert.ok(text.has("fontSize"), "own member");
  assert.ok(text.has("width"), "inherited from CommonMethod");
});

test("a component name maps to its attribute type", () => {
  assert.equal(index.components.get("Text"), "TextAttribute");
});

test("the chain's component is found on one line and across lines", () => {
  assert.equal(chainComponent(["      Text('x')."], 0, 17), "Text");
  assert.equal(chainComponent(["    Column() {", "      Text('x')", "        .fontSize(20)"], 2, 20), "Text");
});
