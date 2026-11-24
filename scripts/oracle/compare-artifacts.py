#!/usr/bin/env python3
"""Compare the artifacts hvigor and hvolt produced for one fixture.

Each class is compared by its own rule: JSON files field by field, `resources.index` byte for
byte, and `modules.abc` through `ark_disasm`, because the binary differs by build metadata that
disassembly drops.

`sourceMaps.map` is judged differently: its `mappings` describe positions in the text the
reference's own emitter produced, while hvolt prints the same program with its own line layout,
so an identical string would measure the printer rather than the map. The map is judged instead
by its claims (the entry keys and the fields a map entry asserts) and by whether `mappings` decode
to positions that exist in the claimed source files and sit at token boundaries. Entries the
reference only has because it generates a file of its own are reported as layout differences.
"""

import json
import pathlib
import shutil
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parents[2]

DISASM_CANDIDATES = [
    pathlib.Path.home() / ".hvolt/sdk",
    pathlib.Path("/Applications/DevEco-Studio.app/Contents/sdk"),
]


def disasm_binary():
    """The first `ark_disasm` this machine can actually run.

    An SDK directory can hold another platform's build (a macOS SDK copied next to a Linux one
    carries a Mach-O `ark_disasm`), and picking it turns every abc comparison into an
    `Exec format error`. The check is the only reliable one: run it once and see.
    """
    for base in DISASM_CANDIDATES:
        if not base.exists():
            continue
        for candidate in sorted(base.rglob("ark_disasm")):
            if not candidate.is_file():
                continue
            try:
                subprocess.run(
                    [str(candidate)],
                    capture_output=True,
                    timeout=30,
                )
            except (OSError, subprocess.SubprocessError):
                continue
            return candidate
    return None


def flatten(value, prefix=""):
    out = {}
    if isinstance(value, dict):
        for key, inner in value.items():
            out.update(flatten(inner, f"{prefix}.{key}" if prefix else key))
    elif isinstance(value, list):
        for index, inner in enumerate(value):
            out.update(flatten(inner, f"{prefix}[{index}]"))
    else:
        out[prefix] = value
    return out


# Fields that name a build-time path. hvigor prefixes them with its own rigid layout
# (`./ets/entryability/...`); hvolt keeps a cleaner tree, so the values may differ as long as
# they still name the same entry file.
LAYOUT_PATH_FIELDS = {"module.abilities[0].srcEntry"}


def same_entry_file(left, right):
    if not isinstance(left, str) or not isinstance(right, str):
        return False
    return left.replace("\\", "/").split("/")[-1] == right.replace("\\", "/").split("/")[-1]


# Fields whose value is the SDK the build was made with, not a decision either compiler made.
# The reference (hvigor on this machine) and hvolt (on the server) resolve their SDKs separately,
# and today those are different builds: 26.0.0.23 Beta1 versus 26.0.0.38 Release. Comparing the
# strings would report that difference once per fixture and hide the fields next to them, so they
# are reported as environment and left out of the verdict. Everything else in the manifest is a
# behaviour and stays compared — including a field one side writes and the other omits.
ENVIRONMENT_FIELDS = (
    ".compileSdkVersion",
    ".apiReleaseType",
    ".apiVersion.releaseType",
    ".apiVersion.version",
)


def is_environment(key):
    return any(key.endswith(suffix) for suffix in ENVIRONMENT_FIELDS)


# Numeric resource ids are assigned by the reference's own resource compiler (restool): the
# numbering is internal, and hvolt's indexer numbers the same resources in a different order. Two
# packages that resolve each reference to its own id are equivalent; comparing the numbers would
# report the numbering, not a behaviour. What is compared instead is the mapping — every id on one
# side must stand for the same reference on the other, and no id may be shared by two references.
ID_FIELDS = ("iconId", "labelId", "descriptionId", "startWindowIconId", "startWindowBackgroundId")


def resource_ids(flat):
    """The partition the ids induce over the references, numbering set aside.

    `{id: [reference, …]}` with the id replaced by its group: two references are equal here when
    the manifest gives them the same id, whatever that id is. That catches an id shared by two
    references (a collision) and a reference that carries no id at all, while leaving the
    numbering itself to the resource compiler.
    """
    groups = {}
    for key, value in flat.items():
        suffix = next((s for s in ID_FIELDS if key.endswith("." + s)), None)
        if suffix is None:
            continue
        groups.setdefault(value, []).append(key[: -len(suffix)] + suffix[: -len("Id")])
    return sorted(tuple(sorted(references)) for references in groups.values())


def compare_json(name, golden, actual, report):
    g = flatten(json.load(open(golden)))
    a = flatten(json.load(open(actual)))
    for key in sorted(set(g) | set(a)):
        before = g.get(key, "<missing>")
        after = a.get(key, "<missing>")
        if before == after:
            continue
        if key in LAYOUT_PATH_FIELDS and same_entry_file(before, after):
            print(f"  info: {name}:{key} differs by layout only ({before} vs {after})")
            continue
        if is_environment(key):
            print(f"  info: {name}:{key} differs by the SDK build ({before} vs {after})")
            continue
        if any(key.endswith("." + suffix) for suffix in ID_FIELDS):
            continue  # judged as a mapping below, not as a number
        report.append((f"{name}:{key}", before, after))
    golden_ids, actual_ids = resource_ids(g), resource_ids(a)
    if golden_ids != actual_ids:
        only_golden = [group for group in golden_ids if group not in actual_ids]
        only_actual = [group for group in actual_ids if group not in golden_ids]
        report.append((f"{name}:resource-id-groups", only_golden, only_actual))
    elif golden_ids:
        print(f"  info: {name}:resource ids follow a different numbering with the same references")


# Records hvolt emits that the reference does not, each with a reason. Anything else is a
# difference, not an allowance.
# Modules the reference emits and hvolt does not, with the reason. Allowed only while the
# produced program never imports them — the check below enforces that.
ALLOWED_MISSING_MODULES = {
    # hvolt resolves `$r('app.string.x')` at transpile time into the object literal the runtime
    # expects, so it never needs the generated resource-table module the reference emits.
    "ResourceTable",
}

ALLOWED_EXTRA_RECORDS = {
    # hvolt's transpiler annotates component classes with an expected-property count; the
    # reference's loader derives the same check at runtime instead.
    "_ESExpectedPropertyCountAnnotation",
}


def hv_dump(path):
    """`hvolt index-dump <path>` as JSON, or None when the binary is not built."""
    binary = ROOT / "target" / "debug" / "hvolt"
    if not binary.is_file():
        return None
    done = subprocess.run(
        [str(binary), "index-dump", str(path)], capture_output=True, text=True
    )
    if done.returncode != 0:
        return None
    try:
        return json.loads(done.stdout)
    except json.JSONDecodeError:
        return None


def index_structure(dump):
    """What a `resources.index` says, with the id numbering set aside.

    Each config becomes `(qualifiers, [(type, name, value-digest)…])`, and the ids become a
    partition over those entries: which entries share an id. The reference's resource compiler
    numbers resources in its own order, so the numbers are not comparable; the partition is.
    """
    configs = []
    for config in dump["configs"]:
        entries = sorted(
            (entry["type"], entry["name"], entry["value"]) for entry in config["entries"]
        )
        groups = {}
        for entry in config["entries"]:
            groups.setdefault(entry["id"], []).append(f"{entry['type']}:{entry['name']}")
        partition = sorted(tuple(sorted(names)) for names in groups.values())
        configs.append(
            (tuple(map(tuple, config["qualifiers"])), tuple(entries), tuple(partition))
        )
    return (dump["version"], tuple(configs))


def compare_index(name, golden, actual, report):
    """`resources.index` by structure: entries and which of them share an id.

    The bytes are the producing tool's layout — string-pool order, offsets, id numbering — and
    two indexes that describe the same resources are equivalent. Judged here only when both
    sides parse; a difference the parser cannot see is caught by the entry and value digests.
    """
    golden_dump, actual_dump = hv_dump(golden), hv_dump(actual)
    if golden_dump is None or actual_dump is None:
        g, a = golden.read_bytes(), actual.read_bytes()
        if g != a:
            report.append((f"{name}:bytes", f"{len(g)} bytes", f"{len(a)} bytes"))
        return
    before, after = index_structure(golden_dump), index_structure(actual_dump)
    if before == after:
        print(f"  info: {name}: same entries, different layout and numbering")
        return
    if before[0] != after[0]:
        report.append((f"{name}:version", before[0], after[0]))
    golden_configs, actual_configs = before[1], after[1]
    for at, (g_cfg, a_cfg) in enumerate(zip(golden_configs, actual_configs)):
        if g_cfg[0] != a_cfg[0]:
            report.append((f"{name}:config[{at}].qualifiers", g_cfg[0], a_cfg[0]))
        if g_cfg[1] != a_cfg[1]:
            only_golden = [e for e in g_cfg[1] if e not in a_cfg[1]]
            only_actual = [e for e in a_cfg[1] if e not in g_cfg[1]]
            report.append((f"{name}:config[{at}].entries", only_golden[:4], only_actual[:4]))
        if g_cfg[2] != a_cfg[2]:
            only_golden = [g for g in g_cfg[2] if g not in a_cfg[2]]
            only_actual = [g for g in a_cfg[2] if g not in g_cfg[2]]
            report.append((f"{name}:config[{at}].id-groups", only_golden[:4], only_actual[:4]))
    if len(golden_configs) != len(actual_configs):
        report.append(
            (f"{name}:configs", len(golden_configs), len(actual_configs))
        )


def compare_bytes(name, golden, actual, report):
    if name == "resources.index":
        compare_index(name, golden, actual, report)
        return
    g = golden.read_bytes()
    a = actual.read_bytes()
    if g != a:
        report.append((f"{name}:bytes", f"{len(g)} bytes", f"{len(a)} bytes"))


def id_labels(manifest):
    """`{id: label}` for the resource ids the manifest names, or `{}` without a manifest.

    The label is the references the id stands for, so the two sides agree on it exactly when
    their numbering differs and their references do not — the same judgement `resource_ids`
    makes for the manifest, applied to the ids embedded in the program's literals.
    """
    if not manifest.is_file():
        return {}
    try:
        flat = flatten(json.load(open(manifest)))
    except (json.JSONDecodeError, OSError):
        return {}
    groups = {}
    for key, value in flat.items():
        suffix = next((s for s in ID_FIELDS if key.endswith("." + s)), None)
        if suffix is None:
            continue
        groups.setdefault(value, []).append(key[: -len(suffix)] + suffix[: -len("Id")])
    return {value: "+".join(sorted(references)) for value, references in groups.items()}


def index_id_labels(index_path, manifest_labels):
    """`{id: "type:name"}` from a `resources.index`, over the manifest's labels.

    The program's literals embed ids that the manifest never names (a `$r('app.string.x')` at a
    use site), so the index — which lists every entry — is the table that can label them. The
    manifest labels stay as a fallback for ids the index does not list.
    """
    dump = hv_dump(index_path) if index_path.is_file() else None
    labels = dict(manifest_labels)
    if dump is None:
        return labels
    for config in dump["configs"]:
        for entry in config["entries"]:
            labels.setdefault(entry["id"], f"{entry['type']}:{entry['name']}")
    return labels


def sdk_versions(golden_manifest, actual_manifest):
    """`(golden, actual)` `compileSdkVersion`, empty when a manifest is missing."""
    versions = []
    for manifest in (golden_manifest, actual_manifest):
        value = ""
        if manifest.is_file():
            try:
                value = flatten(json.load(open(manifest))).get("app.compileSdkVersion", "") or ""
            except (json.JSONDecodeError, OSError):
                value = ""
        versions.append(value)
    return tuple(versions)


def compare_abc(
    name,
    golden,
    actual,
    report,
    golden_labels=None,
    actual_labels=None,
    relax_module_requests=False,
):
    disasm = disasm_binary()
    if not disasm:
        compare_bytes(name, golden, actual, report)
        return
    def dump(path, out):
        subprocess.run([str(disasm), str(path), str(out)], check=False,
                       stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        return out.read_text(encoding="utf-8", errors="replace") if out.exists() else ""
    tmp = actual.parent
    g = dump(golden, tmp / "golden.disasm")
    a = dump(actual, tmp / "actual.disasm")
    if not g or not a:
        compare_bytes(name, golden, actual, report)
        return
    # Three-layer judgement, because a byte-for-byte match is the wrong target: hvolt has its
    # own source layout and inlines `$r()` instead of emitting a ResourceTable module. What has
    # to match is the program: same literal records, same module-level dependencies.
    #
    #   1. literal records: the set of record bodies must be equal (the compiler may allocate
    #      the same body twice, so duplicates are not a difference);
    #   2. module record names: the tail segment must match; a path segment the project layout
    #      does not have (`entryability/`) is reported but not failed;
    #   3. annotation records the reference does not carry are allowed only when registered.
    literal_re = re.compile(r"^\d+ 0x[0-9a-f]+ \{(.*)$")
    record_re = re.compile(r"^\.record (@?&?[^{]+) \{")

    # The `page` field of a debug-info record names the source file the code came from. hvolt
    # compiles its own expanded `.ts` tree, so the value differs by layout; the rest of the
    # record (line, col, and every other key) still has to match.
    # A whole debug-location record is not compared: hvolt transpiles the declarative source
    # into a different file shape and emits no source map, so line/col cannot correspond even
    # in principle. The record's presence is still checked by the set comparison below.
    debug_loc_re = re.compile(
        r'string:"page", string:"[^"]*", string:"line", i32:\d+, string:"col", i32:\d+'
    )

    def strip_module_requests(text: str) -> str:
        """Drop the module-request bookkeeping from a literal body.

        Used only when the two sides were compiled against different SDK versions: a Kit's
        `export *` expands to one request per sub-module the *SDK's* `@kit.*.json` lists, so the
        set of requests follows the SDK, not the source (`fixtures/kit-export-from`: 83 requests
        from 26.0.0.38 against 81 from 26.0.0.23).
        """
        kept = []
        for line in text.splitlines():
            stripped = line.strip()
            if stripped.startswith("ModuleTag:") and "module_request:" in stripped:
                continue
            if re.match(r"^\d+ : @", stripped):
                continue
            kept.append(line)
        return "\n".join(kept)

    def literals(text, labels):
        # A record opening `{` with nothing after it on the same line holds a module request
        # table whose members sit on the following lines. Their count follows the number of
        # module records, so they are not literal data and are compared by record count below.
        bodies = {
            m.group(1).strip()
            for m in (literal_re.match(l) for l in text.splitlines())
            if m and m.group(1).strip() and m.group(1).strip() != "0 ["
        }
        # `lit_offset` is the literal's position inside the file's literal table; the two sides
        # build their strings in a different order, so the offset is layout, not content.
        offset_re = re.compile(r"lit_offset:0x[0-9a-f]+")
        # `#~@N>` is the index of the record a method reference points at. Both sides lay their
        # module records out in their own order, so the index is layout; the referenced name
        # right after it is compared.
        # The separator after the index is `>` for a forward reference and `<` for a local one;
        # both carry the same index.
        record_ref_re = re.compile(r"#~@\d+[<>]")
        # A literal record that is nothing but strings is a set of names; the two sides intern
        # them in the order their source happens to mention them, which is layout.
        string_list_re = re.compile(r'\[ ((?:string:"(?:[^"\\]|\\.)*", )+)\]')

        # A resource id in a literal body is the same numbering the manifest carries; it is
        # normalised to the references it stands for, per side, so two builds that resolve the
        # same resources are equal here even when the compiler numbered them differently
        # (measured on `fixtures/button-create-method`: `i32:16777216` versus `i32:16777218`).
        def normalise_ids(text: str, labels: dict) -> str:
            def replace(match: re.Match) -> str:
                label = labels.get(int(match.group(1)))
                return f"i32:<{label}>" if label else match.group(0)

            return re.sub(r"i32:(\d+)", replace, text)

        def sort_string_list(match: re.Match) -> str:
            items = re.findall(r'string:"(?:[^"\\]|\\.)*"', match.group(1))
            return "[ " + "".join(f"{item}, " for item in sorted(items)) + "]}"

        if relax_module_requests:
            # The module-request table is a record whose body is just its size (`81 [`) with one
            # line per request underneath; the lines are not part of the captured body, so the
            # table is dropped whole rather than edited. Its size follows the SDK's
            # `@kit.*.json`, which is what differs here.
            bodies = {body for body in bodies if not re.fullmatch(r"\d+ \[", body.strip())}
            bodies = {strip_module_requests(body) for body in bodies}
        return {
            string_list_re.sub(sort_string_list,
                record_ref_re.sub("#~@<record>", offset_re.sub("lit_offset:<offset>",
                    debug_loc_re.sub('string:"page", string:"<source>", string:"line", i32:<n>, string:"col", i32:<n>', normalise_ids(b, labels)))))
            for b in bodies
        }

    def records(text):
        return [m.group(1).strip() for m in (record_re.match(l) for l in text.splitlines()) if m]

    glit, alit = literals(g, golden_labels or {}), literals(a, actual_labels or {})
    if glit != alit:
        missing, extra = glit - alit, alit - glit
        report.append((f"{name}:literal-records",
                       f"{len(glit)} distinct, {len(missing)} missing",
                       f"{len(alit)} distinct, {len(extra)} extra"))
        for body in sorted(missing)[:3]:
            print(f"    only golden: {body[:80]}")
        for body in sorted(extra)[:3]:
            print(f"    only actual: {body[:80]}")

    grec, arec = records(g), records(a)
    gtail = {r.replace(".", "/").rstrip("/").split("/")[-1] for r in grec}
    atail = {r.replace(".", "/").rstrip("/").split("/")[-1] for r in arec}
    missing_tails = gtail - atail
    extra_tails = atail - gtail
    unresolved = []
    for r in sorted(missing_tails):
        bare = r.rstrip("&")
        if bare in ALLOWED_MISSING_MODULES:
            if bare in a:
                unresolved.append((f"{name}:missing-record {bare}",
                                   "emitted", "absent but imported"))
            else:
                print(f"    registered missing module: {bare} (never imported)")
            continue
        unresolved.append((f"{name}:missing-record {r}", "emitted", "absent"))
    for r in sorted(extra_tails):
        if r in ALLOWED_EXTRA_RECORDS:
            print(f"    registered extra record: {r}")
        else:
            unresolved.append((f"{name}:extra-record {r}", "<absent>", "present"))
    report.extend(unresolved)

    # `.language` 指令：参考侧对代码生成的 record（`ResourceTable`）标 `TypeScript`，
    # 其余标 `ArkTS`。hvolt 目前一律标 `ArkTS`
    # （`fixtures/import-directory-index`：golden 第 43 行 `TypeScript`，actual `ArkTS`）。
    def languages(text):
        return [line.strip() for line in text.splitlines() if line.startswith(".language")]

    glang, alang = languages(g), languages(a)
    if glang != alang:
        # `.language` 随 SDK 走：golden 由 26.0.0.23 产出、把代码生成的 `ResourceTable` 标
        # `TypeScript`；本机 SDK 是 26.0.0.38，同一份 `export {};` 标 `ArkTS`
        # （159 个 golden 的首个 `.language` 全是 `TypeScript`，actual 全是 `ArkTS`；
        # filesInfo 的语言段两边已一致为 `ts`，staging 扩展名也不改变该指令）。
        for index, (want, got) in enumerate(zip(glang, alang)):
            if want != got:
                print(f"  info: {name}:language[{index}] differs by the SDK build "
                      f"({want} vs {got})")
                break

    if glit == alit and not unresolved:
        print(f"  info: {name} matches at the semantic layers "
              f"(literal bodies and module records)")


SOURCE_MAP = "sourceMaps.map"
# The reference generates the resource table as a file of its own and records a placeholder entry
# for it (empty `mappings`). hvolt inlines the resource table into the page, so that entry has no
# counterpart; requiring one would make the map claim a module that is not in the build. The entry
# is reported as a layout difference instead of being compared.
SOURCE_MAP_LAYOUT_ENTRIES = ("|build/default/generated/r/default/ResourceTable.js",)
# What a map entry claims outright. `mappings` is judged separately, by validity.
SOURCE_MAP_FIELDS = ("version", "file", "sourceRoot", "sources", "names", "entry-package-info")
_VLQ_ALPHABET = {
    char: index
    for index, char in enumerate("ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/")
}
# Characters that continue an identifier, number or keyword. A mapping may point at the start of
# such a token or immediately after one, but not into its middle — that is what makes a shifted
# or truncated `mappings` string detectably wrong rather than merely coarser.
_TOKEN_CHARACTERS = set(
    "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_$"
)


def decode_mappings(mappings):
    """Decode a source-map `mappings` string into its segments.

    A segment is `(generated_line, generated_column, source, source_line, source_column)`;
    `source` and the source position are None for a segment that carries only a generated
    column. Returns None when the string is not valid base64 VLQ.
    """
    segments = []
    source = source_line = source_column = 0
    for generated_line, line in enumerate(mappings.split(";")):
        if not line:
            continue
        generated_column = 0
        for text in line.split(","):
            values = []
            shift = 0
            value = 0
            for character in text:
                digit = _VLQ_ALPHABET.get(character)
                if digit is None:
                    return None
                value += (digit & 31) << shift
                if digit & 32:
                    shift += 5
                    continue
                values.append(-(value >> 1) if value & 1 else value >> 1)
                value = 0
                shift = 0
            if shift or len(values) not in (1, 4, 5):
                return None
            generated_column += values[0]
            if generated_column < 0:
                return None
            if len(values) >= 4:
                source += values[1]
                source_line += values[2]
                source_column += values[3]
                if source < 0 or source_line < 0 or source_column < 0:
                    return None
                segments.append(
                    (generated_line, generated_column, source, source_line, source_column)
                )
            else:
                segments.append((generated_line, generated_column, None, None, None))
    return segments


def is_token_boundary(line, column):
    """Whether a source position sits at a token boundary rather than inside a token."""
    if column == len(line):
        return True
    if column > len(line):
        return False
    if line[column].isspace():
        return column > 0 and not line[column - 1].isspace()
    if column > 0 and line[column - 1] in _TOKEN_CHARACTERS and line[column] in _TOKEN_CHARACTERS:
        return False
    return True


def source_path(fixture, source):
    """Resolve a map `sources` entry to the fixture's own source file.

    The reference layout keeps a module's sources under `<module>/src/main/ets/` and moves the
    entry ability into `entryability/`; the fixture project keeps them under `src/`.
    """
    normalized = source.replace("\\", "/")
    marker = "/src/main/ets/"
    if marker not in normalized:
        return None
    relative = normalized.split(marker, 1)[1]
    project = fixture / "project"
    candidates = [project / "src" / relative]
    if relative.startswith("entryability/"):
        candidates.append(project / "src" / relative[len("entryability/"):])
    for candidate in candidates:
        if candidate.is_file():
            return candidate
    return None


def check_mappings(key, entry, fixture, report):
    """Judge the mappings of one map entry: decodable, in range, at token boundaries."""
    sources = entry.get("sources")
    mappings = entry.get("mappings")
    if not isinstance(sources, list) or not all(isinstance(item, str) for item in sources):
        report.append((f"{SOURCE_MAP}:{key}:mappings", "a list of source files", sources))
        return
    if not isinstance(mappings, str):
        report.append((f"{SOURCE_MAP}:{key}:mappings", "a mappings string", mappings))
        return
    segments = decode_mappings(mappings)
    if segments is None:
        report.append((f"{SOURCE_MAP}:{key}:mappings", "decodable VLQ with non-negative positions", "not decodable"))
        return
    texts = {}
    previous_columns = {}
    for generated_line, generated_column, source, source_line, source_column in segments:
        if generated_line in previous_columns and generated_column <= previous_columns[generated_line]:
            report.append((
                f"{SOURCE_MAP}:{key}:mappings:generated-order",
                f"generated columns increase on line {generated_line}",
                generated_column,
            ))
            return
        previous_columns[generated_line] = generated_column
        if source is None:
            continue
        if source >= len(sources):
            report.append((
                f"{SOURCE_MAP}:{key}:mappings:source-index",
                f"index below {len(sources)}",
                source,
            ))
            return
        path = source_path(fixture, sources[source])
        if path is None:
            report.append((
                f"{SOURCE_MAP}:{key}:mappings:source",
                f"resolvable source file for {sources[source]}",
                "not found",
            ))
            return
        if path not in texts:
            texts[path] = path.read_text(encoding="utf-8", errors="replace").splitlines()
        lines = texts[path]
        if source_line >= len(lines):
            report.append((
                f"{SOURCE_MAP}:{key}:mappings:source-line",
                f"line within {sources[source]} ({len(lines)} lines)",
                source_line,
            ))
            return
        line = lines[source_line]
        if source_column > len(line):
            report.append((
                f"{SOURCE_MAP}:{key}:mappings:source-column",
                f"column within {sources[source]}:{source_line + 1} (line length {len(line)})",
                source_column,
            ))
            return
        if not is_token_boundary(line, source_column):
            report.append((
                f"{SOURCE_MAP}:{key}:mappings:source-position",
                f"token boundary in {sources[source]}:{source_line + 1}",
                f"column {source_column} is inside a token: {line!r}",
            ))
            return


def compare_source_map(golden_path, actual_path, fixture, report, layout):
    """Compare two source maps by their claims and by the validity of their mappings.

    The reference's `mappings` describe positions in the text its own emitter produced; hvolt
    prints the same program with its own line layout, so demanding an identical string would
    only measure the printer. What is judged here instead: the values a map entry asserts
    outright, and whether `mappings` decode to positions that exist in the claimed source files
    and sit at token boundaries.
    """
    try:
        golden = json.loads(golden_path.read_text(encoding="utf-8"))
        actual = json.loads(actual_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        report.append((f"{SOURCE_MAP}:json", "readable JSON on both sides", str(error)))
        return
    if not isinstance(golden, dict) or not isinstance(actual, dict):
        report.append((f"{SOURCE_MAP}:json", "an object of entries", "not an object"))
        return
    for key in sorted(set(golden) | set(actual)):
        if any(key.endswith(suffix) for suffix in SOURCE_MAP_LAYOUT_ENTRIES):
            layout.append(key)
            continue
        if key not in actual or key not in golden:
            report.append((f"{SOURCE_MAP}:{key}:presence", key in golden, key in actual))
            continue
        golden_entry, actual_entry = golden[key], actual[key]
        if not isinstance(golden_entry, dict) or not isinstance(actual_entry, dict):
            report.append((f"{SOURCE_MAP}:{key}", "a map entry object", "not an object"))
            continue
        for field in SOURCE_MAP_FIELDS:
            golden_value = golden_entry.get(field, "<missing>")
            actual_value = actual_entry.get(field, "<missing>")
            if field == "sources":
                # The order a map lists its sources is not a claim; the set is.
                ordered = (
                    isinstance(golden_value, list)
                    and isinstance(actual_value, list)
                    and set(golden_value) == set(actual_value)
                )
                if not ordered:
                    report.append((f"{SOURCE_MAP}:{key}:sources", golden_value, actual_value))
                continue
            if golden_value != actual_value:
                report.append((f"{SOURCE_MAP}:{key}:{field}", golden_value, actual_value))
        check_mappings(key, actual_entry, fixture, report)


def compare_diagnostics(golden_path, actual_path, report):
    """Compare two canonical diagnostic sets line by line.

    The set, not the order: both sides sort. A line only one side has is the whole difference,
    and five of each is enough to say what it is without pasting two full logs into the report.
    """

    def lines(path):
        text = path.read_text(encoding="utf-8", errors="replace")
        out = []
        for line in text.splitlines():
            if not line.strip():
                continue
            # `10905301`（`@Watch` 指向不存在的方法）把方法名放在引号里，两种引号随 SDK 走：
            # 26.0.0.23 的 golden 是 `'onMissing'`，26.0.0.38 的构建是 `"onMissing"`
            # （`fixtures/ui-watch-name` 与 `fixtures/ui-watch-missing-method` 各自固定一侧）。
            # 只在这一条里把双引号折成单引号，其余行原样比较。
            if "10905301" in line:
                line = line.replace('"', "'")
            out.append(line)
        return out

    golden, actual = lines(golden_path), lines(actual_path)
    actual_set, golden_set = set(actual), set(golden)
    for line in [item for item in golden if item not in actual_set][:5]:
        report.append(("diagnostics.txt:unreported", line, None))
    for line in [item for item in actual if item not in golden_set][:5]:
        report.append(("diagnostics.txt:invented", None, line))


def compare_one(name, golden_path, actual_path, report):
    """Compare one present-on-both-sides artifact by its own rule."""
    if name.endswith(".abc"):
        compare_abc(name, golden_path, actual_path, report)
    elif name == SOURCE_MAP:
        # Judged by `compare_source_map`, which reads the fixture's sources.
        raise AssertionError(f"{SOURCE_MAP} has its own comparison")
    elif name.endswith(".json") or name.endswith(".map") or name == "pack.info":
        # `pack.info` is JSON without the extension.
        compare_json(name, golden_path, actual_path, report)
    else:
        compare_bytes(name, golden_path, actual_path, report)


def main():
    fixture = pathlib.Path(sys.argv[1])
    golden = fixture / "golden"
    actual = fixture / "actual"
    if not golden.is_dir() or not actual.is_dir():
        print(f"SKIP {fixture.name}: missing golden or actual")
        return 0
    # Two different judgements are kept apart on purpose:
    #
    # * the artifacts below exist on both sides for every fixture, so a difference is a real
    #   per-fixture defect;
    # * `sourceMaps.map` is a file the reference produces for every fixture and hvolt does not
    #   produce at all. Reporting that absence here would mark every fixture DIFF and erase the
    #   signal of this comparison. It is one gap across the whole oracle and is reported once by
    #   `scripts/oracle/gaps.sh`; when hvolt does emit a map, the map itself is compared below.
    manifest = actual / "manifest.txt"
    produced = None
    if manifest.is_file():
        produced = {
            line.strip()
            for line in manifest.read_text(encoding="utf-8", errors="replace").splitlines()
            if line.strip()
        }
    report = []

    def has_actual(name):
        # A file the build did not produce is absent even if an older build left it behind.
        path = actual / name
        return path.is_file() and (produced is None or name in produced)

    # A fixture whose build must fail has no artifacts on the reference side: the compile stops
    # at the diagnostic, so there is nothing to compare byte for byte. Only the diagnostic set is
    # judged there, and reporting the missing packages would bury it.
    failing = (fixture / "expect-build-failure").is_file()
    for name in () if failing else (
        "module.json",
        "pack.info",
        "pkgContextInfo.json",
        "insight_intent.json",
        "resources.index",
        "modules.abc",
    ):
        gp, ap = golden / name, actual / name
        golden_present = gp.is_file()
        actual_present = has_actual(name)
        if golden_present != actual_present:
            report.append((f"{name}:presence", golden_present, actual_present))
            continue
        if not golden_present:
            continue
        if name.endswith(".abc"):
            compare_abc(
            name,
            gp,
            ap,
            report,
            index_id_labels(golden / "resources.index", id_labels(golden / "module.json")),
            index_id_labels(actual / "resources.index", id_labels(actual / "module.json")),
            sdk_versions(golden / "module.json", actual / "module.json")[0]
            != sdk_versions(golden / "module.json", actual / "module.json")[1],
        )
        elif name == "resources.index":
            compare_bytes(name, gp, ap, report)
        else:
            compare_json(name, gp, ap, report)
    # A golden set harvested before this channel existed has no `diagnostics.txt`; it says
    # nothing about diagnostics, so nothing is judged. Once the golden was harvested with the
    # channel, every line hvolt fails to print is a real difference — including all of them when
    # hvolt prints none.
    gp = golden / "diagnostics.txt"
    if gp.is_file():
        ap = actual / "diagnostics.txt"
        if not ap.is_file():
            ap.write_text("", encoding="utf-8")
        compare_diagnostics(gp, ap, report)

    # Judged only when both sides produce a map, so the current absence stays a single global
    # gap instead of one difference per fixture. When both sides do produce one it is judged
    # semantically, not like the JSON files above: see `compare_source_map`.
    layout = []
    gp, ap = golden / SOURCE_MAP, actual / SOURCE_MAP
    if not failing and gp.is_file() and ap.is_file() and (produced is None or SOURCE_MAP in produced):
        compare_source_map(gp, ap, fixture, report, layout)
    if report:
        print(f"DIFF {fixture.name}: {len(report)} item(s)")
        for key, g, a in report:
            print(f"  {key}\n    golden: {g}\n    actual: {a}")
    else:
        print(f"SAME {fixture.name}")
    for key in layout:
        print(f"  known-layout: {key} (resource-table entry; hvolt inlines the table, so it is not compared)")
    return 1 if report else 0


sys.exit(main())
