// The hvolt.toml key list in the language server must match what the parser accepts —
// `members` was missing and every project that listed its modules got a warning on a
// valid key (measured on /tmp/hv-showcase, hvolt.toml line 23).
import assert from "node:assert/strict";
import test from "node:test";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { validateToml } from "../dist/tomlcheck.mjs";

const configRs = readFileSync(fileURLToPath(new URL("../../src/config.rs", import.meta.url)), "utf8");

/** The toml key of every field in a Rust struct, following `#[serde(rename = …)]`. */
function tomlKeys(structName) {
  const body = new RegExp(`pub struct ${structName} \\{([\\s\\S]*?)\\n\\}`).exec(configRs);
  assert.ok(body, `${structName} not found in src/config.rs`);
  const keys = new Set();
  const pending = [];
  for (const line of body[1].split("\n")) {
    const trimmed = line.trim();
    if (trimmed.startsWith("#[serde")) {
      pending.push(trimmed);
    } else if (trimmed.startsWith("pub ")) {
      const field = /pub (\w+):/.exec(trimmed)[1];
      const renamed = pending.map((attribute) => /rename = "([^"]+)"/.exec(attribute)?.[1]).find(Boolean);
      const skips = pending.some((attribute) => attribute.includes("skip"));
      if (!skips) keys.add(renamed ?? field);
      pending.length = 0;
    }
  }
  return keys;
}

test("no key AppFields accepts is reported as unknown", () => {
  const expected = tomlKeys("AppFields");
  const text = [...expected].map((key) => `${key} = ${key.endsWith("s") ? "[]" : '"x"'}`).join("\n");
  // Every key the parser knows must pass validation; an unknown one must not.
  const clean = validateToml(`[app]\n${text}\n`);
  assert.deepEqual(
    clean.map((diagnostic) => diagnostic.message),
    [],
    "a key AppFields accepts is reported as unknown",
  );
  const unknown = validateToml("[app]\nnot_a_key = 1\n");
  assert.equal(unknown.length, 1, "an unknown key must still be reported");
});

test("a [module] section accepts the keys ModuleInfo takes, `type` included", () => {
  // `module_type` is renamed to `type` on the toml side; the checker must not offer
  // `module_type` instead.
  const text = [
    "[module]",
    'name = "src"',
    'entry = "EntryAbility.ets"',
    'type = "entry"',
    'device_types = ["phone"]',
    'routes = ["pages/Index"]',
    "",
  ].join("\n");
  assert.deepEqual(validateToml(text), []);
});

test("the sampled project's [app] section validates", () => {
  // Copied from the showcase: `members` is what used to trip the checker.
  const text = [
    "[app]",
    'bundle = "com.example.hello_world"',
    'name = "hello-world"',
    'version = "1.0.0"',
    'vendor = "example"',
    'icon = "$media:layered_image"',
    'sdk_version = "26.0.0"',
    'compile_sdk_version = "26.0.0"',
    'compatible_sdk_version = ""',
    'bundle_type = "app"',
    'products = ["default"]',
    'runtime_os = "HarmonyOS"',
    'members = ["src"]',
    "",
    "[app.signing]",
    "",
  ].join("\n");
  assert.deepEqual(validateToml(text), []);
});
