#!/usr/bin/env python3
"""Turn one build log into the fixture's diagnostic set, in one canonical form.

Both sides of the oracle print diagnostics in their own shape: the reference writes them as
ANSI-coloured blocks inside the hvigor log (`1 ERROR: 10605008 ArkTS Compiler Error` followed by
`Error Message: ... At File: <abs path>:<line>:<col>`), hvolt prints its own lines. Comparing the
raw text would compare two printers. What is compared instead is one line per diagnostic:

    <severity> <code> <file>:<line>:<col> <message>

`severity` is `ERROR` or `WARN`; `code` is the numeric hvigor code or `-` when the message carries
none; `file` is the path relative to the module's `src/main` when it can be recognised, else the
basename; `line`/`col` are `-` when the diagnostic names no position. The set is sorted, so the
order the compiler happened to report in is not part of the judgement.

Usage:
    diagnostics.py <log file>
    diagnostics.py --status <log file>    # `fail` / `ok`, from the build's own verdict line
"""

import re
import sys

# CSI sequences: colour is presentation, not content.
ANSI = re.compile(r"\x1b\[[0-9;]*m")
# `1 ERROR: 10605008 ArkTS Compiler Error` / `2 WARN: 10905301 ...`
HEAD = re.compile(r"^\s*(\d+)\s+(ERROR|WARN):\s*(\d{3,})?\s*(.*)$")
# `Error Message: <message> At File: <path>:<line>:<col>`
BODY = re.compile(r"^\s*Error Message:\s*(.*?)\s*(?:At File:\s*(.*):(\d+):(\d+))?\s*$")
# `Error Message: <message> At File: <path>` — a diagnostic the compiler reports against the whole
# file, with no line or column (`10905231`, `10905402`).
BODY_FILEONLY = re.compile(r"^\s*Error Message:\s*(.*?)\s*At File:\s*(\S+)\s*$")
# `At File: <path>:<line>:<col>` at the end of a message — the positioned shape.
POSITIONED_AT = re.compile(r"At File:\s*\S+:\d+:\d+\s*$")
# `2 ERROR: ArkTS:ERROR File: <path>:<line>:<col>` — the shape that carries the position in the
# head and the code on its own `ERROR Code:` line.
HEAD_POSITIONED = re.compile(r"^\s*\d+\s+(ERROR|WARN):\s*ArkTS:(?:ERROR|WARN)\s+File:\s*(.*):(\d+):(\d+)\s*$")
CODE_LINE = re.compile(r"^\s*ERROR Code:\s*(\d{3,})")
# `ArkTS:WARN: <message>` / `ArkTS:ERROR: <message>` — the shape without a code or a position.
ARKTS = re.compile(r"^\s*ArkTS:(ERROR|WARN):\s*(.+)$")
# A line that is already in this file's own form, as hvolt prints it. Taken verbatim: it is the
# canonical shape, so parsing it again would only risk normalising a path twice.
CANONICAL = re.compile(r"^(ERROR|WARN) (?:\d{3,}|-) \S+:(?:\d+|-):(?:\d+|-) .+$")
RESULT = re.compile(r"COMPILE RESULT:(OK|FAIL)\s*\{ERROR:(\d+)(?:\s*WARN:(\d+))?\}")


def strip_ansi(text):
    return ANSI.sub("", text)


def relative_path(path):
    """`/…/entry/src/main/ets/pages/Index.ets` → `ets/pages/Index.ets`."""
    if not path:
        return "-"
    normalised = path.replace("\\", "/")
    marker = "/src/main/"
    if marker in normalised:
        return normalised.split(marker, 1)[1]
    return normalised.rsplit("/", 1)[-1]


def parse(text):
    lines = strip_ansi(text).splitlines()
    seen = set()
    for index, line in enumerate(lines):
        if CANONICAL.match(line.strip()):
            seen.add(line.strip())
            continue
        positioned = HEAD_POSITIONED.match(line)
        if positioned:
            severity = positioned.group(1)
            where = relative_path(positioned.group(2))
            line_no, column = positioned.group(3), positioned.group(4)
            code = "-"
            message = ""
            for follow in lines[index + 1 : index + 5]:
                code_match = CODE_LINE.match(follow)
                if code_match:
                    code = code_match.group(1)
                    continue
                body = BODY.match(follow)
                if body:
                    message = " ".join(body.group(1).split())
                    break
                # The linter's own shape puts the text on a bare line under the position:
                #   `1 WARN: ArkTS:WARN File: <path>:9:3`
                #   ` Definite assignment assertions are not supported (arkts-no-…)`
                if follow.strip() and not follow.strip().startswith(">"):
                    message = " ".join(follow.split())
                    break
            seen.add(f"{severity} {code} {where}:{line_no}:{column} {message}".strip())
            continue
        head = HEAD.match(line)
        if head:
            code = head.group(3) or "-"
            severity = head.group(2)
            # The message is on the following `Error Message:` line; a head with nothing after it
            # is the section title (`ERROR: ArkTS Compiler Error`), not a diagnostic.
            # A message can wrap: `This expression is not callable.` continues on the next line
            # with the type, and only the last line carries `At File:`. Joining the lines up to
            # that one keeps the full text; taking one line truncated it (measured on
            # `fixtures/member-collision`).
            joined = ""
            for follow in lines[index + 1 : index + 9]:
                joined = f"{joined} {follow}".strip()
                if "At File:" in joined:
                    break
            # `BODY`'s `At File:` group is optional, so it also matches a message that ends in an
            # `At File:` without a position; the two shapes are told apart by the position itself.
            positioned = POSITIONED_AT.search(joined) if joined else None
            body = BODY.match(joined) if positioned else None
            fileonly = None if positioned else (BODY_FILEONLY.match(joined) if joined else None)
            if not body:
                # A numbered entry with no position and no `Error Message:` line still counts:
                # the reference prints `1 WARN: ArkTS:WARN: For details about ArkTS syntax
                # errors, see FAQs` and its own tally line counts it (`{ERROR:3 WARN:1}`).
                inline = ARKTS.match(head.group(4))
                if inline:
                    message = " ".join(inline.group(2).split())
                    seen.add(f"{inline.group(1)} - -:-:- {message}")
            if fileonly:
                # No position at all: the reference names only the file.
                message = " ".join(fileonly.group(1).split())
                where = relative_path(fileonly.group(2))
                # `file:-:-` — the same two-field shape hvolt prints for a file-level entry.
                seen.add(f"{severity} {code} {where}:-:-  {message}".replace("  ", " "))
            if body:
                message = " ".join(body.group(1).split())
                where = relative_path(body.group(2))
                line_no = body.group(3) or "-"
                column = body.group(4) or "-"
                seen.add(f"{severity} {code} {where}:{line_no}:{column} {message}")
            continue
        arkts = ARKTS.match(line)
        if arkts:
            message = " ".join(arkts.group(2).split())
            seen.add(f"{arkts.group(1)} - -:-:- {message}")
    return sorted(seen)


def status(text):
    found = RESULT.search(strip_ansi(text))
    if not found:
        return "unknown"
    return "fail" if found.group(1) == "FAIL" else "ok"


def main(argv):
    if len(argv) < 2:
        print(__doc__.strip().splitlines()[-1], file=sys.stderr)
        return 2
    if argv[1] == "--status":
        text = open(argv[2], encoding="utf-8", errors="replace").read()
        print(status(text))
        return 0
    text = open(argv[1], encoding="utf-8", errors="replace").read()
    for line in parse(text):
        print(line)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))