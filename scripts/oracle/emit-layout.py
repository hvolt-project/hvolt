#!/usr/bin/env python3
"""Write the hvigor-compatible layout of a hvolt project into a directory.

The repository holds hvolt projects only (`hvolt.toml` + `src/` + `resources/`). When the
oracle needs a reference build, this script materialises the equivalent standard layout in a
temporary directory so hvigor can consume it; nothing it writes belongs in the repository.

A project may declare more than one member in `[app].members` (an entry module plus a shared
HSP, say). Each member directory carries its own `hvolt.toml`, its own `resources/`, and its
own module shape: the member whose `[module].type` is `shared`/`hsp` is emitted as a library
module (`hspTasks`, `module.json5` with `"type": "shared"`), every other member as an entry
module (`hapTasks`). The module list, with the hvigor module name and task kind, is written
next to the layout as `<out>.modules.json` so the caller can drive one hvigor invocation per
module without re-parsing the hvolt sources.

Usage: emit-layout.py <hvolt-project-dir> <out-dir>
"""

import json
import pathlib
import shutil
import sys
import tomllib

APP_SDK_TARGET = "6.0.0(20)"
# hvolt spells a shared package either way; hvigor's module type is `shared` in both cases.
HSP_TYPES = {"shared", "hsp"}


def load(path):
    with open(path, "rb") as handle:
        return tomllib.load(handle)


def write_json(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")


def read_modules(src_root, app_cfg):
    """Resolve every member of `[app].members` to its own `[module]` table."""
    members = app_cfg.get("members") or ["src"]
    modules = []
    for member in members:
        cfg = load(src_root / member / "hvolt.toml")["module"]
        modules.append({
            "member": member,
            "name": cfg.get("name", "entry"),
            "type": cfg.get("type", ""),
            "entry": cfg.get("entry", "EntryAbility.ets"),
            "routes": [r if r.endswith(".ets") else f"{r}.ets" for r in cfg.get("routes", [])],
            "device_types": cfg.get("device_types", ["phone", "tablet"]),
            "module_cfg": cfg,
        })
    return modules


def module_resources(src_root, member):
    """A member's own `resources/`; a single-module project keeps them at the project root."""
    own = src_root / member / "resources"
    return own if own.is_dir() else src_root / "resources"


def staged_packages(src_root, member):
    """Packages a fixture installs under `project/oh_modules` (or `<member>/oh_modules`).

    hvigor resolves `oh_modules` from the module directory, so the packages are copied next to
    the module and named in the generated manifest: a package the manifest does not list is not
    resolvable, so a fixture could not import one at all.
    """
    root = src_root / member / "oh_modules"
    if not root.is_dir():
        root = src_root / "oh_modules"
    packages = {}
    if not root.is_dir():
        return packages
    for entry_dir in sorted(root.iterdir()):
        if not entry_dir.is_dir() or entry_dir.name.startswith("."):
            continue
        # Scoped names (`@demo/kinds`) sit one level down.
        if entry_dir.name.startswith("@"):
            for scoped in sorted(entry_dir.iterdir()):
                if not scoped.is_dir():
                    continue
                packages[f"{entry_dir.name}/{scoped.name}"] = scoped
            continue
        packages[entry_dir.name] = entry_dir
    return packages


def emit_shared(src_root, out, mod, app_cfg):
    """A shared (HSP) member: `hspTasks`, `type: shared`, source at `src/main/`."""
    name = mod["name"]
    mod_root = out / name
    mod_root.mkdir(parents=True, exist_ok=True)
    entry = mod["entry"]
    (mod_root / "hvigorfile.ts").write_text(
        "import { hspTasks } from '@ohos/hvigor-ohos-plugin';\n"
        "export default { system: hspTasks, plugins: [] };\n", encoding="utf-8")
    write_json(mod_root / "build-profile.json5",
               {"apiType": "stageMode", "buildOption": {}, "targets": [{"name": "default"}]})

    main_out = mod_root / "src" / "main"
    main_out.mkdir(parents=True, exist_ok=True)
    # The library's source root is `src/main`; its entry file is declared in `srcEntry` and the
    # package's `main` points at it, which is the shape a DevEco shared module has.
    for source in (src_root / mod["member"]).rglob("*"):
        if not source.is_file() or source.suffix not in {".ets", ".ts"}:
            continue
        if source.name.endswith((".d.ets", ".d.ts")):
            continue
        rel = source.relative_to(src_root / mod["member"])
        target = main_out / rel
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source, target)
    shutil.copy2(src_root / mod["member"] / entry, main_out / entry)

    res_src = module_resources(src_root, mod["member"])
    if res_src.is_dir():
        shutil.copytree(res_src, main_out / "resources", dirs_exist_ok=True)

    packages = staged_packages(src_root, mod["member"])
    write_json(mod_root / "oh-package.json5", {
        "name": name, "version": "1.0.0", "description": "",
        "main": f"src/main/{entry}",
        "author": "", "license": "Apache-2.0",
        "dependencies": {pkg: "1.0.0" for pkg in packages},
    })
    for pkg, source in packages.items():
        shutil.copytree(source, mod_root / "oh_modules" / pkg, dirs_exist_ok=True)

    write_json(main_out / "module.json5", {"module": {
        "name": name,
        "type": "shared",
        "description": "$string:module_desc",
        "srcEntry": f"./{entry}",
        "deviceTypes": mod["device_types"],
        "deliveryWithInstall": True,
        "installationFree": False,
    }})


def emit_entry(src_root, out, mod, app_cfg, hsp_names):
    """An entry (HAP) member: `hapTasks`, abilities, pages, and its HSP dependencies."""
    name = mod["name"]
    mod_root = out / name
    entry = mod["entry"]
    (mod_root / "src" / "main" / "ets").mkdir(parents=True, exist_ok=True)
    (mod_root / "hvigorfile.ts").write_text(
        "import { hapTasks } from '@ohos/hvigor-ohos-plugin';\n"
        "export default { system: hapTasks, plugins: [] };\n", encoding="utf-8")
    write_json(mod_root / "build-profile.json5",
               {"apiType": "stageMode", "buildOption": {}, "targets": [{"name": "default"}]})

    packages = staged_packages(src_root, mod["member"])
    # A shared member is reached through the module manifest, not through `oh_modules`: the
    # dependency is a sibling module of the same project, so the specifier is a relative path.
    dependencies = {pkg: "1.0.0" for pkg in packages}
    for shared_name in hsp_names:
        dependencies[shared_name] = f"file:../{shared_name}"
    write_json(mod_root / "oh-package.json5", {
        "name": name, "version": "1.0.0", "description": "", "main": "",
        "author": "", "license": "Apache-2.0",
        "dependencies": dependencies,
    })
    for pkg, source in packages.items():
        shutil.copytree(source, mod_root / "oh_modules" / pkg, dirs_exist_ok=True)

    ets_out = mod_root / "src" / "main" / "ets"
    entry_file = src_root / mod["member"] / entry
    (ets_out / "entryability").mkdir(parents=True, exist_ok=True)
    shutil.copy2(entry_file, ets_out / "entryability" / entry)
    for route in mod["routes"]:
        target = ets_out / route
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(src_root / mod["member"] / route, target)
    # Sources that are neither the entry ability nor a page still exist in a real project: a
    # helper or component module is reached only through the import graph. They are copied
    # verbatim so the reference builds the same source tree; a file no entry reaches stays in
    # the layout and must stay out of the compiled abc (the `unused-source` fixture).
    copied = {entry, *mod["routes"]}
    for source in (src_root / mod["member"]).rglob("*"):
        if not source.is_file() or source.suffix not in {".ets", ".ts"}:
            continue
        rel = source.relative_to(src_root / mod["member"])
        if rel.as_posix() in copied or rel.name.endswith((".d.ets", ".d.ts")):
            continue
        target = ets_out / rel
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source, target)

    res_src = module_resources(src_root, mod["member"])
    res_out = mod_root / "src" / "main" / "resources"
    if res_src.is_dir():
        shutil.copytree(res_src, res_out, dirs_exist_ok=True)
    (res_out / "base" / "profile").mkdir(parents=True, exist_ok=True)
    write_json(res_out / "base" / "profile" / "main_pages.json",
               {"src": [r[: -len(".ets")] for r in mod["routes"]]})

    write_json(mod_root / "src" / "main" / "module.json5", {"module": {
        "name": name,
        "type": "entry",
        "description": "$string:module_desc",
        "mainElement": entry[: -len(".ets")],
        "deviceTypes": mod["device_types"],
        "deliveryWithInstall": True,
        "installationFree": False,
        "pages": "$profile:main_pages",
        # A project that ships a router-map profile declares it here. Without this key hvigor
        # never reads `route_map.json`, so `createNavigationInit` has no entries and emits no
        # `NavigationBuilderRegister` IIFE — measured by adding the key by hand to a scratch
        # layout and watching the tail of the emitted file grow one.
        **({"routerMap": "$profile:route_map"}
           if (res_src / "base" / "profile" / "route_map.json").exists() else {}),
        # A HAP that consumes a shared module names it here; the runtime resolves the package
        # from this list, and the packer copies it into the packed `module.json`.
        **({"dependencies": [
            {"bundleName": app_cfg["bundle"], "moduleName": shared_name, "versionCode": 1000000}
            for shared_name in hsp_names
        ]} if hsp_names else {}),
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


def main():
    src_root = pathlib.Path(sys.argv[1]).resolve()
    out = pathlib.Path(sys.argv[2]).resolve()
    app_cfg = load(src_root / "hvolt.toml")["app"]
    modules = read_modules(src_root, app_cfg)
    hsp_names = [m["name"] for m in modules if m["type"] in HSP_TYPES]

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
            "name": mod["name"],
            "srcPath": f"./{mod['name']}",
            "targets": [{"name": "default", "applyToProducts": ["default"]}],
        } for mod in modules],
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

    for mod in modules:
        if mod["type"] in HSP_TYPES:
            emit_shared(src_root, out, mod, app_cfg)
        else:
            emit_entry(src_root, out, mod, app_cfg, hsp_names)

    # A `.hap` resolves its `.hsp` through `oh_modules`, the same way a package is resolved: the
    # manifest names the dependency (`"file:../shared"`) and the link is what `ohpm install`
    # would have created. Without it the ArkTS compile reports `"shared" is imported by ... but
    # could not be resolved – treating it as an external dependency` and then `Cannot find
    # module 'shared'`; with it, the entry build compiles the shared module's own tasks and
    # links against its abc instead of inlining its sources.
    for mod in modules:
        if mod["type"] in HSP_TYPES:
            continue
        for shared_name in hsp_names:
            link = out / mod["name"] / "oh_modules" / shared_name
            link.parent.mkdir(parents=True, exist_ok=True)
            if link.exists() or link.is_symlink():
                continue
            link.symlink_to(f"../../{shared_name}")

    # The caller runs one hvigor invocation per module and needs the hvigor module name, the
    # task that packages it, and the artifact extension that task produces.
    write_json(out.parent / f"{out.name}.modules.json", {"modules": [{
        "name": mod["name"],
        "dir": mod["name"],
        "kind": "shared" if mod["type"] in HSP_TYPES else "entry",
        "task": "assembleHsp" if mod["type"] in HSP_TYPES else "assembleHap",
        "extension": "hsp" if mod["type"] in HSP_TYPES else "hap",
    } for mod in modules]})
    print(f"layout: {out}")


main()