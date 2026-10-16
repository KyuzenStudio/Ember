#!/usr/bin/env python3
"""Report Rust source files/functions exceeding ENGINEERING_STANDARDS.md limits.

Usage:
    python3 scripts/check_source_limits.py [--strict] [--root DIR]

Counts code lines (excludes blank lines, `//` line comments, and the
`#[cfg(test)] mod tests` block, which is measured against test limits).
Function bodies found by naive brace matching: NOT a full Rust parser,
macro-generated and cfg-heavy code may misattribute. Treat function
numbers as review pointers, not ground truth.

Exit code: 0 always, unless --strict and a NON-EXCEPTED file exceeds its
review threshold. Excepted files (ENGINEERING_STANDARDS.md section 8)
are reported but never fail.
"""

import argparse
import pathlib
import re
import sys

FILE_THRESHOLD_SRC = 500
FILE_THRESHOLD_TESTS = 600
FN_REVIEW = 60
FN_STRONG = 100

# Reviewed exceptions, mirrors ENGINEERING_STANDARDS.md section 8.
EXCEPTIONS = {
    "crates/ember-editor/src/ui.rs",
    "crates/ember-vm/src/vm.rs",
    "crates/ember-runtime/src/runtime.rs",
    "crates/ember-lang/src/compile.rs",
    "crates/ember-lang/src/semantic.rs",
    "crates/ember-lang/src/parser.rs",
}

SKIP_DIRS = {".git", "target"}


def test_mod_span(lines: list) -> tuple:
    """Return (start, end) line indexes of `#[cfg(test)] mod tests`, or None."""
    for i, line in enumerate(lines):
        if line.strip() == "#[cfg(test)]" and i + 1 < len(lines) and "mod tests" in lines[i + 1]:
            depth = 0
            j = i + 1
            started = False
            while j < len(lines):
                for ch in lines[j]:
                    if ch == "{":
                        depth += 1
                        started = True
                    elif ch == "}":
                        depth -= 1
                if started and depth <= 0:
                    return (i, j)
                j += 1
    return None


def code_lines(path: pathlib.Path) -> int:
    lines = path.read_text(encoding="utf-8").split("\n")
    span = test_mod_span(lines)
    n = 0
    for idx, line in enumerate(lines):
        if span is not None and span[0] <= idx <= span[1]:
            continue
        s = line.strip()
        if s and not s.startswith("//"):
            n += 1
    return n


def functions(path: pathlib.Path):
    lines = path.read_text(encoding="utf-8").split("\n")
    out = []
    i = 0
    while i < len(lines):
        m = re.match(r"\s*(?:pub(?:\([^)]*\))?\s+)?fn\s+(\w+)", lines[i])
        if m:
            depth = 0
            j = i
            started = False
            while j < len(lines):
                for ch in lines[j]:
                    if ch == "{":
                        depth += 1
                        started = True
                    elif ch == "}":
                        depth -= 1
                if started and depth <= 0:
                    break
                j += 1
            out.append((m.group(1), i + 1, j - i + 1))
            i = j + 1
        else:
            i += 1
    return out


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--strict", action="store_true")
    ap.add_argument("--root", default=".")
    args = ap.parse_args()

    root = pathlib.Path(args.root)
    bad_files = []
    big_fns = []
    for path in sorted(root.rglob("*.rs")):
        if any(part in SKIP_DIRS for part in path.parts):
            continue
        rel = path.relative_to(root).as_posix()
        is_test = "tests" in pathlib.PurePosixPath(rel).parts or rel.endswith("/tests.rs")
        limit = FILE_THRESHOLD_TESTS if is_test else FILE_THRESHOLD_SRC
        n = code_lines(path)
        if n > limit:
            tag = "EXCEPTED" if rel in EXCEPTIONS else "VIOLATION"
            print(f"[{tag}] {rel}: {n} code lines (threshold {limit})")
            if rel not in EXCEPTIONS:
                bad_files.append(rel)
        for name, line, size in functions(path):
            if size >= FN_REVIEW:
                mark = "!" if size >= FN_STRONG else " "
                print(f"[fn{mark}] {rel}:{line}: {name} ({size} lines)")
                if size >= FN_STRONG:
                    big_fns.append((rel, name))
    if not bad_files and not big_fns:
        print("ok: no files over threshold, no functions over strong threshold")
    if args.strict and bad_files:
        print(f"STRICT FAIL: {len(bad_files)} non-excepted file(s) over threshold")
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
