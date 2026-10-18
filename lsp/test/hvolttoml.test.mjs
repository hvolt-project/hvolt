// Unit tests for the hvolt.toml intelligence (key completion, route extraction).
import assert from "node:assert/strict";
import test from "node:test";
import { completeKeys, declaredRoutes, inRouterCall } from "../dist/hvolttoml.mjs";

const APP = `[app]
bundle = "com.example.app"
name = "app"
`;

const MODULE = `[module]
name = "src"
entry = "EntryAbility.ets"
routes = ["pages/Index.ets", "pages/Detail.ets"]
device_types = ["phone"]
`;

test("app section completes app keys", () => {
  const keys = completeKeys(APP, 2).map((k) => k.key);
  assert.ok(keys.includes("bundle"));
  assert.ok(keys.includes("sdk_version"));
  assert.ok(!keys.includes("routes"));
});

test("module section completes module keys", () => {
  const keys = completeKeys(MODULE, 4).map((k) => k.key);
  assert.ok(keys.includes("routes"));
  assert.ok(keys.includes("device_types"));
  assert.ok(!keys.includes("bundle"));
});

test("routes are read from the file", () => {
  assert.deepEqual(declaredRoutes(MODULE), ["pages/Index.ets", "pages/Detail.ets"]);
  assert.deepEqual(declaredRoutes(APP), []);
});

test("router calls are recognised", () => {
  assert.ok(inRouterCall("    router.pushUrl({ url: 'pages/Index.ets' })"));
  assert.ok(inRouterCall("      url: "));
  assert.ok(!inRouterCall("      Text('hello')"));
});
