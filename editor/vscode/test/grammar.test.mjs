// Tokenises a sample with the real TextMate engine, so the grammar is verified by
// behaviour rather than by eyeballing. VS Code supplies `source.ts`; the test
// supplies a small stand-in for it and checks only the ArkTS-specific rules.
import assert from "node:assert/strict";
import test from "node:test";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import textmate from "vscode-textmate";
import onig from "vscode-oniguruma";

const { Registry } = textmate;

const dir = fileURLToPath(new URL("..", import.meta.url));
const ets = JSON.parse(readFileSync(join(dir, "syntaxes", "ets.tmLanguage.json"), "utf8"));

const TYPESCRIPT_STUB = {
  scopeName: "source.ts",
  comment: "stand-in for VS Code's built-in TypeScript grammar",
  patterns: [{ include: "#keyword" }],
  repository: { keyword: { match: "\\b(const|let|string|number)\\b", name: "storage.type.ts" } },
};

const grammar = (scopeName) => (scopeName === "source.ets" ? ets : TYPESCRIPT_STUB);

const wasm = readFileSync(
  join(dir, "node_modules", "vscode-oniguruma", "release", "onig.wasm"),
).buffer;

const registry = new Registry({
  onigLib: (onig.default ?? onig).loadWASM(wasm).then(() => ({
    createOnigScanner: (patterns) => new (onig.default ?? onig).OnigScanner(patterns),
    createOnigString: (text) => new (onig.default ?? onig).OnigString(text),
  })),
  loadGrammar: async (scopeName) => grammar(scopeName),
});

const SAMPLE = `@Entry
@Component
struct Index {
  @State message: string = $r('app.string.title');

  build() {
    Column() {
      Text(this.message)
    }.width('100%')
  }
}
`;

function scopesOf(source) {
  const found = new Map();
  return (async () => {
    const g = await registry.loadGrammar("source.ets");
    let rule = (onig.default ?? onig).INITIAL;
    for (const line of source.split("\n")) {
      const result = g.tokenizeLine(line, rule);
      for (const token of result.tokens) {
        const text = line.slice(token.startIndex, token.endIndex).trim();
        if (text) found.set(text, token.scopes.join(" "));
      }
      rule = result.ruleStack;
    }
    return found;
  })();
}

test("ArkTS-specific forms get their own scopes", async () => {
  const scopes = await scopesOf(SAMPLE);
  assert.match(scopes.get("@Entry") ?? "", /meta\.decorator\.arkts/);
  assert.match(scopes.get("@Component") ?? "", /meta\.decorator\.arkts/);
  assert.match(scopes.get("@State") ?? "", /meta\.decorator\.arkts/);
  assert.match(scopes.get("struct") ?? "", /storage\.type\.struct\.arkts/);
  assert.match(scopes.get("Column") ?? "", /support\.class\.arkui/);
  assert.match(scopes.get("Text") ?? "", /support\.class\.arkui/);
  const resource = [...scopes.entries()].filter(([text]) => text.includes("$r("));
  assert.ok(resource.length > 0, "$r(…) did not tokenise at all");
  assert.ok(
    resource.some(([, scope]) => /constant\.other\.resource\.arkts/.test(scope)),
    `$r(…) scopes were ${resource.map(([, scope]) => scope).join(" | ")}`,
  );
});

test("everything else falls through to the TypeScript grammar", async () => {
  const scopes = await scopesOf(SAMPLE);
  assert.match(scopes.get("string") ?? "", /storage\.type\.ts|storage\.type/);
});
