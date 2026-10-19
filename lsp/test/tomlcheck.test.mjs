// Unit tests for hvolt.toml validation and dependency navigation.
import assert from "node:assert/strict";
import test from "node:test";
import { dependencyAt, dependencyTarget, scan, validateToml } from "../dist/tomlcheck.mjs";

const GOOD = `[app]
bundle = "com.example.app"
name = "app"
sdk_version = "26.0.0"

[module]
name = "src"
routes = ["pages/Index.ets"]
device_types = ["phone"]
`;

test("a valid file produces no diagnostics", () => {
  assert.deepEqual(validateToml(GOOD, { routeExists: () => true }), []);
});

test("unknown keys are flagged with their position", () => {
  const found = validateToml(GOOD.replace('name = "app"', 'nmae = "app"'));
  assert.equal(found.length, 1);
  assert.match(found[0].message, /unknown key 'nmae'/);
  assert.equal(found[0].line, 2);
  assert.equal(found[0].character, 0);
});

test("missing routes are errors", () => {
  const found = validateToml(GOOD, { routeExists: (route) => route === "pages/Index.ets" });
  assert.equal(found.length, 0);
  const broken = validateToml(GOOD.replace('pages/Index.ets', 'pages/Missing.ets'), { routeExists: () => false });
  assert.equal(broken.length, 1);
  assert.match(broken[0].message, /does not exist/);
  assert.equal(broken[0].severity, "error");
});

test("device_types must be an array", () => {
  const found = validateToml(GOOD.replace('device_types = ["phone"]', 'device_types = "phone"'));
  assert.equal(found.length, 1);
  assert.match(found[0].message, /must be an array/);
});

test("scanner reports sections and values", () => {
  const entries = scan(GOOD);
  assert.equal(entries.find((e) => e.key === "bundle")?.section, "app");
  assert.equal(entries.find((e) => e.key === "routes")?.section, "module");
});

test("dependency path is located under the cursor and resolved", () => {
  const text = `[dependencies]
libhello = { path = "mylib", version = "1.0.0" }
`;
  assert.equal(dependencyAt(text, 1, 24), "mylib");
  assert.equal(dependencyAt(text, 1, 2), null);
  assert.equal(dependencyTarget("/p/hvolt.toml", "mylib"), "/p/mylib/hvolt.toml");
});

test("multi-line arrays are collected, not flagged as a type error", () => {
  const text = `[module]
name = "src"
routes = [
    "pages/Index.ets",
    "pages/Detail.ets",
]
device_types = [
    "phone",
]
`;
  const found = validateToml(text, { routeExists: (route) => route === "pages/Index.ets" });
  assert.equal(found.filter((d) => /must be an array/.test(d.message)).length, 0);
  assert.equal(found.filter((d) => /does not exist/.test(d.message)).length, 1);
  const routes = scan(text).find((e) => e.key === "routes");
  assert.equal(routes?.line, 2, "the entry keeps the line where the key starts");
});
