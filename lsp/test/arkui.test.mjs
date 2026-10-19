// ArkUI attribute completion is served from the SDK declarations, because the fork's
// parser rejects component block-chains (`Text('x').` answered 0 candidates).
import assert from "node:assert/strict";
import test from "node:test";
import { createRequire } from "node:module";
import { buildAttributeIndex, buildTypeIndex, chainCallAt, chainComponent, membersOf, parameterTypes } from "../dist/arkui.mjs";

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

// Arguments of a component attribute are declared objects; the language service cannot
// parse the chain, so these lookups are the only source of completion inside them.
const COMMON_EXT = `declare interface LocalizedAlignRuleOptions {
  center?: LocalizedHorizontalAlignParam;
  bias?: Bias;
}
declare interface LocalizedHorizontalAlignParam {
  anchor: string;
  align: HorizontalAlign;
}
declare interface Bias { horizontal: number; }`;

const index2 = buildTypeIndex(ts, new Map([["/sdk/component/common.d.ts", COMMON_EXT]]));

test("a point-typed parameter resolves to its declared members", () => {
  const signature = "alignRules(alignRule: LocalizedAlignRuleOptions): T";
  const params = parameterTypes(signature);
  assert.deepEqual(params, ["LocalizedAlignRuleOptions"]);
  const top = membersOf(index2, params[0], []);
  assert.deepEqual([...top.keys()], ["center", "bias"]);
  const nested = membersOf(index2, params[0], ["center"]);
  assert.deepEqual([...nested.keys()], ["anchor", "align"]);
});

test("the cursor's position in a chain call is recovered", () => {
  // Regression: these regexes were double-escaped once, so every lookup returned [].
  assert.deepEqual(chainCallAt("      .alignRules({"), { member: "alignRules", argument: 0, keys: [] });
  assert.deepEqual(chainCallAt("      .alignRules({ center: {"), { member: "alignRules", argument: 0, keys: ["center"] });
  assert.equal(chainCallAt("      .width(100)"), undefined, "a closed call is not an open argument list");
});
