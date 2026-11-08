#!/usr/bin/env python3
"""Write the hvigor-compatible layout of a hvolt project into a directory.

The repository holds hvolt projects only (`hvolt.toml` + `src/` + `resources/`). When the
oracle needs a reference build, this script materialises the equivalent standard layout in a
temporary directory so hvigor can consume it; nothing it writes belongs in the repository.

Usage: emit-layout.py <hvolt-project-dir> <out-dir>
"""

import json
import pathlib
import shutil
import sys
import tomllib

APP_SDK_TARGET = "6.0.0(20)"


def load(path):
    with open(path, "rb") as handle:
        return tomllib.load(handle)


def write_json(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")


def main():
    src_root = pathlib.Path(sys.argv[1]).resolve()
    out = pathlib.Path(sys.argv[2]).resolve()
    app_cfg = load(src_root / "hvolt.toml")["app"]
    members = app_cfg.get("members") or ["src"]

    module_dir = members[0]
    module_cfg = load(src_root / module_dir / "hvolt.toml")["module"]
    module_name = module_cfg.get("name", "entry")
    routes = [r if r.endswith(".ets") else f"{r}.ets" for r in module_cfg.get("routes", [])]
    entry = module_cfg.get("entry", "EntryAbility.ets")

    out.mkdir(parents=True, exist_ok=True)
    # hvigor scaffolding
    (out / "hvigorfile.ts").write_text(
        "import { appTasks } from '@ohos/hvigor-ohos-plugin';\n"
        "export default { system: appTasks, plugins: [] };\n", encoding="utf-8")
    (out / "hvigor").mkdir(exist_ok=True)
    write_json(out / "hvigor" / "hvigor-config.json5", {"modelVersion": "5.0.0", "dependencies": {}})
    write_json(out / "oh-package.json5", {
        "modelVersion": "5.0.0",
        "name": app_cfg.get("name", "app"),
        "version": app_cfg.get("version", "1.0.0"),
        "description": "",
        "main": "",
        "author": app_cfg.get("vendor", ""),
        "license": "Apache-2.0",
        "dependencies": {},
        "devDependencies": {},
    })
    write_json(out / "build-profile.json5", {
        "app": {
            "products": [{
                "name": "default",
                "compatibleSdkVersion": app_cfg.get("compatible_sdk_version") or APP_SDK_TARGET,
                "targetSdkVersion": app_cfg.get("target_sdk_version") or APP_SDK_TARGET,
                "runtimeOS": app_cfg.get("runtime_os", "HarmonyOS"),
                "buildOption": {
                    "strictMode": {"caseSensitiveCheck": True, "useNormalizedOHMUrl": True},
                },
            }],
            "buildModeSet": [{"name": "debug"}, {"name": "release"}],
        },
        "modules": [{
            "name": module_name,
            "srcPath": f"./{module_name}",
            "targets": [{"name": "default", "applyToProducts": ["default"]}],
        }],
    })

    # AppScope
    write_json(out / "AppScope" / "app.json5", {"app": {
        "bundleName": app_cfg["bundle"],
        "vendor": app_cfg.get("vendor", "example"),
        "versionCode": 1000000,
        "versionName": app_cfg.get("version", "1.0.0"),
        "icon": app_cfg.get("icon", "$media:layered_image"),
        "label": "$string:app_name",
    }})
    app_res = out / "AppScope" / "resources" / "base"
    app_res.mkdir(parents=True, exist_ok=True)
    write_json(app_res / "element" / "string.json",
               {"string": [{"name": "app_name", "value": app_cfg.get("name", "app")}]})
    icon = app_cfg.get("icon", "$media:layered_image")
    media_name = icon.split(":", 1)[-1] if ":" in icon else "app_icon"
    src_media = src_root / "resources" / "base" / "media"
    app_media = app_res / "media"
    app_media.mkdir(exist_ok=True)
    # The app scope carries the icon exactly as the project composes it: a layered image
    # moves together with the media it references, a plain image moves alone.
    layered = src_media / f"{media_name}.json"
    if layered.is_file():
        shutil.copy2(layered, app_media / layered.name)
        composed = [tok.split('"', 1)[0].strip() for tok in layered.read_text().split('"$media:')[1:]]
        for name in composed:
            png = src_media / f"{name}.png"
            if name and png.is_file():
                shutil.copy2(png, app_media / png.name)
    else:
        png = src_media / f"{media_name}.png"
        if png.is_file():
            shutil.copy2(png, app_media / png.name)

    # module
    mod_root = out / module_name
    (mod_root / "src" / "main" / "ets").mkdir(parents=True, exist_ok=True)
    (mod_root / "hvigorfile.ts").write_text(
        "import { hapTasks } from '@ohos/hvigor-ohos-plugin';\n"
        "export default { system: hapTasks, plugins: [] };\n", encoding="utf-8")
    write_json(mod_root / "build-profile.json5", {"apiType": "stageMode", "buildOption": {}, "targets": [{"name": "default"}]})
    # Packages the fixture installs under `project/oh_modules` are copied next to the module,
    # where hvigor resolves `oh_modules` from, and named in the generated manifest: a package
    # the manifest does not list is not resolvable, so a fixture could not import one at all.
    # A fixture without that directory gets exactly the layout it got before (empty map, no copy).
    package_root = src_root / "oh_modules"
    packages = {}
    if package_root.is_dir():
        for entry_dir in sorted(package_root.iterdir()):
            if not entry_dir.is_dir() or entry_dir.name.startswith("."):
                continue
            # Scoped names (`@demo/kinds`) sit one level down.
            if entry_dir.name.startswith("@"):
                for scoped in sorted(entry_dir.iterdir()):
                    if not scoped.is_dir():
                        continue
                    packages[f"{entry_dir.name}/{scoped.name}"] = str(scoped)
                continue
            packages[entry_dir.name] = str(entry_dir)
    write_json(mod_root / "oh-package.json5", {
        "name": module_name, "version": "1.0.0", "description": "", "main": "",
        "author": "", "license": "Apache-2.0",
        "dependencies": {name: "1.0.0" for name in packages},
    })
    if packages:
        staged_packages = mod_root / "oh_modules"
        for name, source in packages.items():
            target = staged_packages / name
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copytree(source, target, dirs_exist_ok=True)

    ets_out = mod_root / "src" / "main" / "ets"
    entry_file = src_root / module_dir / entry
    (ets_out / "entryability").mkdir(parents=True, exist_ok=True)
    shutil.copy2(entry_file, ets_out / "entryability" / "EntryAbility.ets")
    for route in routes:
        target = ets_out / route
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(src_root / module_dir / route, target)
    # Sources that are neither the entry ability nor a page still exist in a real project: a
    # helper or component module is reached only through the import graph. They are copied
    # verbatim so the reference builds the same source tree; a file no entry reaches stays in
    # the layout and must stay out of the compiled abc (the `unused-source` fixture).
    copied = {entry, *routes}
    for source in (src_root / module_dir).rglob("*"):
        if not source.is_file() or source.suffix not in {".ets", ".ts"}:
            continue
        rel = source.relative_to(src_root / module_dir)
        if rel.as_posix() in copied or rel.name.endswith((".d.ets", ".d.ts")):
            continue
        target = ets_out / rel
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source, target)

    res_out = mod_root / "src" / "main" / "resources"
    shutil.copytree(src_root / "resources", res_out, dirs_exist_ok=True)
    (res_out / "base" / "profile").mkdir(parents=True, exist_ok=True)
    write_json(res_out / "base" / "profile" / "main_pages.json",
               {"src": [r[: -len(".ets")] for r in routes]})

    write_json(mod_root / "src" / "main" / "module.json5", {"module": {
        "name": module_name,
        "type": "entry",
        "description": "$string:module_desc",
        "mainElement": entry[: -len(".ets")],
        "deviceTypes": module_cfg.get("device_types", ["phone", "tablet"]),
        "deliveryWithInstall": True,
        "installationFree": False,
        "pages": "$profile:main_pages",
        # A project that ships a router-map profile declares it here. Without this key hvigor
        # never reads `route_map.json`, so `createNavigationInit` has no entries and emits no
        # `NavigationBuilderRegister` IIFE — measured by adding the key by hand to a scratch
        # layout and watching the tail of the emitted file grow one.
        **({"routerMap": "$profile:route_map"}
           if (src_root / "resources" / "base" / "profile" / "route_map.json").exists() else {}),
        "abilities": [{
            "name": entry[: -len(".ets")],
            "srcEntry": f"./ets/entryability/{entry}",
            "description": "$string:EntryAbility_desc",
            "icon": app_cfg.get("icon", "$media:layered_image"),
            "label": "$string:EntryAbility_label",
            "startWindowIcon": "$media:startIcon",
            "startWindowBackground": "$color:start_window_background",
            "exported": True,
            "skills": [{"entities": ["entity.system.home"], "actions": ["action.system.home"]}],
        }],
    }})
    print(f"layout: {out}")


main()
