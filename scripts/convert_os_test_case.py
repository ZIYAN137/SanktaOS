#!/usr/bin/env python3
"""
Convert os crate `test_case!(name, { ... });` macros to `#[test_case] fn name() { ... }`.

This script is intentionally conservative:
- It only converts the simple form `test_case!(name, {` on its own line.
- It only converts a closing `});` when it is inside a converted test_case block.
- If `test_case!` remains after conversion, it reports an error.
"""

from __future__ import annotations

import argparse
import pathlib
import re
import sys


OPEN_RE = re.compile(
    r"^(?P<indent>[ \t]*)test_case!\((?P<name>[A-Za-z0-9_]+),\s*\{\s*$"
)


def convert_text(text: str, path: pathlib.Path) -> str:
    lines = text.splitlines(keepends=True)
    out: list[str] = []
    stack = 0

    for line in lines:
        m = OPEN_RE.match(line.rstrip("\n"))
        if m:
            ind = m.group("indent")
            name = m.group("name")
            out.append(f"{ind}#[test_case]\n")
            out.append(f"{ind}fn {name}() {{\n")
            stack += 1
            continue

        stripped = line.strip()
        if stripped == "});" and stack > 0:
            out.append(line[: len(line) - len(line.lstrip(" \t"))] + "}\n")
            stack -= 1
            continue

        out.append(line)

    if stack != 0:
        raise RuntimeError(f"{path}: unbalanced test_case! blocks (stack={stack})")

    s = "".join(out)

    # Normalize common import forms.
    s = re.sub(
        r"use crate::\{\s*kassert\s*,\s*test_case\s*\};",
        "use crate::kassert;",
        s,
    )
    s = re.sub(
        r"use crate::\{\s*test_case\s*,\s*kassert\s*\};",
        "use crate::kassert;",
        s,
    )

    # Ensure no blank line between attribute and fn.
    s = re.sub(r"(?m)^(\s*#\[test_case\])\n\s*\n(\s*fn\s+)", r"\1\n\2", s)

    # Fail fast if we left any test_case! in the file.
    if "test_case!(" in s:
        raise RuntimeError(f"{path}: remaining test_case!(...) after conversion")

    return s


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("paths", nargs="+", help="Files to convert in-place")
    args = ap.parse_args()

    for p in [pathlib.Path(x) for x in args.paths]:
        orig = p.read_text(encoding="utf-8")
        new = convert_text(orig, p)
        if new != orig:
            p.write_text(new, encoding="utf-8")

    return 0


if __name__ == "__main__":
    sys.exit(main())

