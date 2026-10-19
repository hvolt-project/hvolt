// The manifest and the code must agree: every command the extension contributes has
// to be registered, and the language id has to match the document selector.
import assert from "node:assert/strict";
import test from "node:test";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const dir = fileURLToPath(new URL("..", import.meta.url));
const manifest = JSON.parse(readFileSync(join(dir, "package.json"), "utf8"));
const source = readFileSync(join(dir, "src", "extension.ts"), "utf8");

test("every contributed command is registered by the extension", () => {
  const contributed = manifest.contributes.commands.map((c) => c.command);
  const registered = /const COMMANDS = \[([^\]]+)\]/.exec(source);
  assert.ok(registered, "COMMANDS list not found");
  const listed = [...registered[1].matchAll(/"([^"]+)"/g)].map((m) => m[1]);
  for (const command of contributed) {
    assert.ok(listed.includes(command), `${command} is contributed but not registered`);
  }
  for (const command of listed) {
    assert.ok(contributed.includes(command), `${command} is registered but not contributed`);
  }
});

test("the contributed language matches the document selector", () => {
  const language = manifest.contributes.languages[0];
  assert.equal(language.id, "ets");
  assert.ok(language.extensions.includes(".ets"));
  assert.match(source, /language: "ets"/);
  assert.match(source, /hvolt\.toml/);
});

test("activation happens on ets files or a hvolt project", () => {
  assert.ok(manifest.activationEvents.includes("onLanguage:ets"));
  assert.ok(manifest.activationEvents.includes("workspaceContains:hvolt.toml"));
});

test("the client launches `hvolt lsp --stdio` from the configured binary", () => {
  assert.match(source, /\["lsp", "--stdio"\]/);
  const binary = manifest.contributes.configuration.properties["hvolt.binaryPath"];
  assert.equal(typeof binary.default, "string");
  assert.ok(binary.default.length > 0);
});
