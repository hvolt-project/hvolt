// hvolt.toml intelligence: key completion and route extraction.
//
// The schema lives in Rust (src/config.rs); this mirrors the documented surface.
// Routes are read straight out of the file so `router.pushUrl('pages/…')` can
// complete from what the project actually declares.

export interface TomlKey {
  key: string;
  detail: string;
}

const APP_KEYS: TomlKey[] = [
  { key: "bundle", detail: "Bundle name (com.example.app)" },
  { key: "name", detail: "Display name" },
  { key: "version", detail: "Version name" },
  { key: "vendor", detail: "Vendor string" },
  { key: "icon", detail: "Icon resource (e.g. $media:startIcon)" },
  { key: "bundle_type", detail: 'app | atomicService' },
  { key: "sdk_version", detail: "SDK the project targets (e.g. 26.0.0)" },
  { key: "compile_sdk_version", detail: "SDK used to compile" },
  { key: "compatible_sdk_version", detail: "Minimum compatible SDK (empty = sdk_version)" },
  { key: "runtime_os", detail: 'HarmonyOS | OpenHarmony' },
  { key: "products", detail: 'Products to build (default ["default"])' },
  { key: "sdk_path", detail: "Explicit SDK install path" },
];

const MODULE_KEYS: TomlKey[] = [
  { key: "name", detail: "Module directory name" },
  { key: "entry", detail: "Entry ability file (EntryAbility.ets)" },
  { key: "routes", detail: "Page routes for main_pages.json" },
  { key: "device_types", detail: "default | phone | tablet | tv | wearable | car | 2in1" },
  { key: "skills", detail: "Entry ability skills" },
  { key: "abilities", detail: "Additional abilities" },
  { key: "permissions", detail: "Runtime permissions" },
  { key: "obfuscation", detail: "Enable code obfuscation" },
  { key: "abi_filters", detail: 'Native ABIs (default ["arm64-v8a"])' },
  { key: "type", detail: 'hsp | shared for non-entry modules' },
];

/** Keys for the section the cursor is in. */
export function completeKeys(text: string, line: number): TomlKey[] {
  let section = "app";
  for (let i = 0; i < line; i++) {
    const match = /^\s*\[([A-Za-z_]+)\]/.exec(text.split(/\r?\n/)[i] ?? "");
    if (match) section = match[1];
  }
  return section === "module" ? MODULE_KEYS : APP_KEYS;
}

/** Routes declared in this file (`routes = ["pages/Index.ets", …]`). */
export function declaredRoutes(text: string): string[] {
  const routes: string[] = [];
  const match = /routes\s*=\s*\[([\s\S]*?)\]/.exec(text);
  if (!match) return routes;
  for (const quoted of match[1].matchAll(/"([^"]+)"/g)) routes.push(quoted[1]);
  return routes;
}

/** True when the line looks like a router call's string argument. */
export function inRouterCall(lineText: string): boolean {
  return /router\.(pushUrl|replaceUrl)\s*\(/.test(lineText) || /url\s*:\s*$/.test(lineText.trim());
}
