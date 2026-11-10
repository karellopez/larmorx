# SPDX-License-Identifier: Apache-2.0
"""Relay 2c: larmorx-gpl's golden cases re-recorded with `<version>` differ by that string only.

The gpl golden harness (`crates-gpl/larmorx-gpl-cli/tests/golden.rs`) writes larmorx-gpl's own
version as `<version>` in `dataset_description.json`'s "Python" field. Given the record before
(`--old`, recorded at 0.0.1), the record after (`--new`) and the outputs the harness saved for the
cases that failed before the re-recording (`--outputs`, its `LARMORX_GOLDEN_MISMATCHES` folder),
this shows:

1. the two records hold the same outputs, and the lines that differ are exactly the
   `dataset_description.json` outputs (each case's one), nothing else;
2. for each of them, the saved file, its `Node` block dropped as the harness drops it, hashes to
   the old record's SHA-256 once the version it names is written as the old one (`0.0.1`), and to
   the new record's once it is written as `<version>`: the only difference is that string, and
   the second check also shows that this script drops the `Node` block as the harness does.

Usage:
    python -P gpl_golden_version_only.py --old OLD.tsv --new NEW.tsv --outputs DIR
        [--old-version 0.0.1]
Exit status 0 when every check holds; the last line is `GPL-VERSION-ONLY ok` or `... FAILED`.
"""

from __future__ import annotations

import argparse
import hashlib
import re
import sys
from pathlib import Path

PYTHON = re.compile(r'"Python": "larmorx-gpl (\S+) ')


def read_tsv(path: Path) -> dict[tuple[str, str], str]:
    """`(case, output) -> sha256` of a golden.tsv."""
    out = {}
    for line in path.read_text(encoding="utf-8").splitlines():
        if line.startswith("#") or not line.strip():
            continue
        case, output, sha, _dtype, _sample = line.split("\t")
        out[case, output] = sha
    return out


def without_node(text: str) -> str:
    """The harness's `without_node`: the `"Node": {` block (to its closing brace at the same
    indentation) left out, every other line kept with a newline (Rust's `str::lines`)."""
    lines = text.split("\n")
    if lines and lines[-1] == "":
        lines.pop()
    out, depth = [], None
    for raw in lines:
        line = raw.removesuffix("\r")
        stripped = line.lstrip()
        indent = len(line) - len(stripped)
        if depth is not None:
            if stripped.startswith("}") and indent == depth:
                depth = None
            continue
        if stripped.startswith('"Node": {'):
            depth = indent
            continue
        out.append(line + "\n")
    return "".join(out)


def sha256(text: str) -> str:
    return hashlib.sha256(text.encode("utf-8")).hexdigest()


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n", 1)[0])
    ap.add_argument("--old", type=Path, required=True, help="golden.tsv before the re-recording")
    ap.add_argument("--new", type=Path, required=True, help="golden.tsv after it")
    ap.add_argument("--outputs", type=Path, required=True, help="the harness's mismatch folder")
    ap.add_argument("--old-version", default="0.0.1", help="the version --old was recorded at")
    args = ap.parse_args(argv)
    old, new = read_tsv(args.old), read_tsv(args.new)
    root = args.outputs / "larmorx-gpl" if (args.outputs / "larmorx-gpl").is_dir() else args.outputs
    problems = []
    if old.keys() != new.keys():
        problems.append(
            f"different outputs: only old {sorted(old.keys() - new.keys())}, "
            f"only new {sorted(new.keys() - old.keys())}"
        )
    changed = sorted(k for k in old.keys() & new.keys() if old[k] != new[k])
    expected = sorted(k for k in old if k[1].endswith("dataset_description.json"))
    print(f"{len(old)} outputs recorded; {len(changed)} changed")
    if changed != expected:
        problems.append(f"changed {changed}, expected the description outputs {expected}")
    for case, output in changed:
        path = root / case / output
        if not path.is_file():
            problems.append(f"{case} {output}: no saved output at {path}")
            continue
        text = without_node(path.read_text(encoding="utf-8"))
        named = PYTHON.findall(text)
        if len(named) != 1:
            problems.append(f"{case} {output}: names larmorx-gpl's version {len(named)} times")
            continue
        (version,) = named
        before = f'"Python": "larmorx-gpl {version} '
        as_old = sha256(text.replace(before, f'"Python": "larmorx-gpl {args.old_version} '))
        as_new = sha256(text.replace(before, '"Python": "larmorx-gpl <version> '))
        ok_old, ok_new = as_old == old[case, output], as_new == new[case, output]
        print(
            f"{case} {output}: names {version}; as {args.old_version}: "
            f"{'= old record' if ok_old else 'DIFFERS from the old record'}; as <version>: "
            f"{'= new record' if ok_new else 'DIFFERS from the new record'}"
        )
        if not (ok_old and ok_new):
            problems.append(f"{case} {output}: old {ok_old}, new {ok_new}")
    for p in problems:
        print("PROBLEM:", p)
    print("GPL-VERSION-ONLY", "FAILED" if problems else "ok")
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
