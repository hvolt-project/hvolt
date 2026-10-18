// Unit tests for the diagnostic parsing/mapping, using output captured from real
// tools (a codelinter report and a hvigor compile failure).
import assert from "node:assert/strict";
import test from "node:test";
import { fromLint, mapToProject, parseHvigor } from "../dist/diagnostics.mjs";

const ROOT = "/home/u/proj";

test("lint-tree paths map back to the flat layout", () => {
  const file =
    "/home/u/.hvolt/lint/tree/proj-0f84f148c3b3c7a3/src/src/main/ets/entryability/EntryAbility.ets";
  assert.equal(mapToProject(file, ROOT), "/home/u/proj/src/entryability/EntryAbility.ets");
});

test("build-tree paths map back to the flat layout", () => {
  const file = "/home/u/proj/build/src/src/main/ets/pages/Index.ets";
  assert.equal(mapToProject(file, ROOT), "/home/u/proj/src/pages/Index.ets");
});

test("paths outside the project are ignored", () => {
  assert.equal(mapToProject("/opt/sdk/ets/api/@ohos.app.ability.UIAbility.d.ts", ROOT), null);
});

test("real hvigor failure is parsed (captured 2026-10-19)", () => {
  const sample = `
> hvigor ERROR: ArkTS Compiler Error
1 ERROR: 10505001 ArkTS Compiler Error
Error Message: Cannot find name 'noSuchVariable'. At File: /home/hongyuncong/work/hvolt-verify/examples/ohos-demo/build/src/src/main/ets/pages/Index.ets:8:12
COMPILE RESULT:FAIL {ERROR:2}
`;
  const found = parseHvigor(sample, "/home/hongyuncong/work/hvolt-verify/examples/ohos-demo");
  assert.equal(found.length, 1);
  assert.equal(found[0].path, "/home/hongyuncong/work/hvolt-verify/examples/ohos-demo/src/pages/Index.ets");
  assert.equal(found[0].line, 8);
  assert.equal(found[0].column, 12);
  assert.equal(found[0].message, "Cannot find name 'noSuchVariable'");
  assert.equal(found[0].severity, "error");
});

test("codelinter defects are mapped and severity is translated", () => {
  const report = {
    defects: [
      {
        engine: "eslintAgent",
        file: "/home/u/.hvolt/lint/tree/proj-abc123/src/src/main/ets/entryability/EntryAbility.ets",
        line: 1,
        column: 50,
        message: "Strings must use doublequote.",
        rule: "@typescript-eslint/quotes",
        severity: 2,
      },
    ],
  };
  const found = fromLint(report, ROOT);
  assert.equal(found.length, 1);
  assert.equal(found[0].path, "/home/u/proj/src/entryability/EntryAbility.ets");
  assert.equal(found[0].severity, "warning");
  assert.match(found[0].message, /@typescript-eslint\/quotes/);
});
