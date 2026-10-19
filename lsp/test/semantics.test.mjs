// Semantic tokens come from the declarations the server indexes, because the SDK's
// classifier aborts on ArkUI syntax.
import assert from "node:assert/strict";
import test from "node:test";
import { TOKEN_TYPES, encode, semanticTokens } from "../dist/semantics.mjs";

const SOURCE = [
  "@Entry",
  "@Component",
  "struct Index {",
  "  @State message: string = 'Hello';",
  "",
  "  build() {",
  "    Column() {",
  "      Text(this.message).fontSize(20)",
  "    }",
  "  }",
  "}",
].join("\n");

const indexes = {
  components: new Set(["Column", "Text"]),
  structs: new Set(["Index"]),
  enums: new Map([["FontWeight", ["Bold"]]]),
  decorators: new Set(["Entry", "Component", "State"]),
  functions: new Set(["$r"]),
  attributes: (component) => new Set(component === "Text" ? ["fontSize"] : []),
};

const named = (tokens, source = SOURCE) =>
  tokens.map((token) => `${source.split(/\r?\n/)[token.line].slice(token.character, token.character + token.length)}:${TOKEN_TYPES[token.type]}`);

test("decorators, the struct, components, attributes and members are classified", () => {
  const found = named(semanticTokens(SOURCE, indexes, (line) => (line === 7 ? "Text" : undefined)));
  assert.deepEqual(found, [
    "Entry:decorator",
    "Component:decorator",
    "Index:class",
    "State:decorator",
    "message:property",
    "build:method",
    "Column:class",
    "Text:class",
    "message:property",
    "fontSize:method",
  ]);
});

test("an enum member is classified through its enum", () => {
  const source = "struct A {\n  build() { Text('x').fontWeight(FontWeight.Bold) }\n}";
  const found = semanticTokens(source, indexes, () => undefined).map(
    (token) => `${source.split("\n")[token.line].slice(token.character, token.character + token.length)}:${TOKEN_TYPES[token.type]}`,
  );
  assert.ok(found.includes("FontWeight:enum"), `missing enum: ${found}`);
  assert.ok(found.includes("Bold:enumMember"), `missing enum member: ${found}`);
});

test("comments and plain names are left to the grammar", () => {
  const source = "// @Entry Text\nstruct Index {\n  build() { const plain = 1; }\n}";
  const found = named(semanticTokens(source, indexes, () => undefined), source);
  // `plain` is a local, and locals belong to the grammar; the struct is ours.
  assert.deepEqual(found, ["Index:class", "build:method"]);
});

test("a module specifier is not a name", () => {
  // Reported from the editor: `@kit` inside `'@kit.AbilityKit'` was coloured as a
  // decorator and `AbilityKit` as a property, because the classifier read the raw line.
  const source = [
    "import { AbilityConstant, UIAbility, Want } from '@kit.AbilityKit';",
    "import { hilog } from '@kit.PerformanceAnalysisKit';",
    "import { window } from '@kit.ArkUI';",
  ].join("\n");
  assert.deepEqual(semanticTokens(source, indexes, () => undefined), []);
});

test("strings, template literals and comments are left to the grammar", () => {
  const source = [
    "struct Index {",
    "  @State message: string = 'Text in a string'; // Text in a comment",
    "  build() {",
    "    console.log(`${Text}`);",
    "  }",
    "}",
  ].join("\n");
  const found = named(semanticTokens(source, indexes, () => undefined), source);
  // `console.log` is real code, so `log` is classified (a member access); the `Text`
  // inside the string and the comment must not appear.
  assert.deepEqual(found, ["Index:class", "State:decorator", "message:property", "build:method", "log:property"]);
});

test("encoding is delta pairs from the previous token", () => {
  const data = encode([
    { line: 0, character: 1, length: 3, type: 1 },
    { line: 0, character: 6, length: 2, type: 2 },
    { line: 2, character: 0, length: 4, type: 3 },
  ]);
  assert.deepEqual(data, [0, 1, 3, 1, 0, 0, 5, 2, 2, 0, 2, 0, 4, 3, 0]);
});
