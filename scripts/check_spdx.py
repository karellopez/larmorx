# SPDX-License-Identifier: Apache-2.0
"""Check that every source file carries the SPDX licence header of its licence family.

docs/licensing.md: each licence family lives in its own directories, and every source file
names its licence on one of its first lines (after a shebang):

- Apache-2.0 directories (`crates/`, `python/`, `tests/`, `validation/`, `scripts/`): the
  expression must contain `Apache-2.0` and must not contain GPL or FSL terms. Files
  translated from MIT or BSD code say so, e.g. `Apache-2.0 AND MIT`.
- `crates-gpl/`: the expression must start with `GPL-3.0-or-later`.
- `crates-nc/`: the expression must contain `LicenseRef-FSL`.

Usage:
    python scripts/check_spdx.py          # report files without a correct header; exit 1 if any
    python scripts/check_spdx.py --fix    # add the header to Apache-2.0 files that lack one
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SUFFIXES = {".rs": "//", ".py": "#", ".pyi": "#", ".sh": "#"}
APACHE_DIRS = ("crates", "python", "tests", "validation", "scripts")
SKIP_PARTS = {"target", ".venv", "__pycache__", "build", "dist", ".git"}
SPDX = re.compile(r"SPDX-License-Identifier:\s*(.+?)\s*$")

# Apache-2.0 files that are translations of code under another permissive licence. The
# original notice is reproduced in each file and in NOTICE.
MIXED = {
    "crates/larmorx-core/src/math/exp.rs": "Apache-2.0 AND MIT",
    "crates/larmorx-core/src/math/log.rs": "Apache-2.0 AND MIT",
    "crates/larmorx-core/src/math/sin.rs": "Apache-2.0 AND MIT",
    "crates/larmorx-core/src/math/cos.rs": "Apache-2.0 AND MIT",
    "crates/larmorx-core/src/math/dint.rs": "Apache-2.0 AND MIT",
    "crates/larmorx-core/src/vnl_svd.rs": "Apache-2.0 AND BSD-3-Clause",
    "crates/larmorx-interp/src/vnl.rs": "Apache-2.0 AND BSD-3-Clause",
    "crates/larmorx-interp/src/ndimage/mod.rs": "Apache-2.0 AND BSD-3-Clause",
    "crates/larmorx-interp/src/ndimage/splines.rs": "Apache-2.0 AND BSD-3-Clause",
    "crates/larmorx-interp/src/ndimage/geometric.rs": "Apache-2.0 AND BSD-3-Clause",
    "crates/larmorx-transform/src/nitransforms.rs": "Apache-2.0 AND MIT",
    "crates/larmorx-transform/src/openblas.rs": "Apache-2.0 AND BSD-3-Clause",
    "crates/larmorx-transform/src/resample_series.rs": "Apache-2.0 AND MIT",
    "python/larmorx/ndimage.py": "Apache-2.0 AND BSD-3-Clause",
    "python/larmorx/transforms/chain.py": "Apache-2.0 AND MIT",
    "python/larmorx/transforms/resample.py": "Apache-2.0 AND MIT",
}


def family(rel: Path) -> str | None:
    top = rel.parts[0]
    if top in APACHE_DIRS:
        return "apache"
    if top == "crates-gpl":
        return "gpl"
    if top == "crates-nc":
        return "nc"
    return None


def header(path: Path) -> str | None:
    with path.open(encoding="utf-8") as f:
        for _, line in zip(range(3), f, strict=False):
            m = SPDX.search(line)
            if m:
                return m.group(1)
    return None


def problem(fam: str, expr: str | None) -> str | None:
    if expr is None:
        return "no SPDX-License-Identifier in the first 3 lines"
    if fam == "apache" and ("Apache-2.0" not in expr or "GPL" in expr or "FSL" in expr):
        return f"'{expr}' is not an Apache-2.0 expression"
    if fam == "gpl" and not expr.startswith("GPL-3.0-or-later"):
        return f"'{expr}' does not start with GPL-3.0-or-later"
    if fam == "nc" and "LicenseRef-FSL" not in expr:
        return f"'{expr}' does not name LicenseRef-FSL"
    return None


def add_header(path: Path, expr: str) -> None:
    text = path.read_text(encoding="utf-8")
    line = f"{SUFFIXES[path.suffix]} SPDX-License-Identifier: {expr}\n"
    if text.startswith("#!"):
        first, _, rest = text.partition("\n")
        text = f"{first}\n{line}{rest}"
    else:
        text = line + text
    path.write_text(text, encoding="utf-8")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--fix", action="store_true", help="add missing Apache-2.0 headers")
    args = parser.parse_args()
    bad = []
    for path in sorted(ROOT.rglob("*")):
        rel = path.relative_to(ROOT)
        if path.suffix not in SUFFIXES or not path.is_file() or SKIP_PARTS & set(rel.parts):
            continue
        fam = family(rel)
        if fam is None:
            continue
        expr = header(path)
        if args.fix and expr is None and fam == "apache":
            add_header(path, MIXED.get(rel.as_posix(), "Apache-2.0"))
            expr = header(path)
        msg = problem(fam, expr)
        if msg is None and rel.as_posix() in MIXED and expr != MIXED[rel.as_posix()]:
            msg = f"'{expr}' should be '{MIXED[rel.as_posix()]}'"
        if msg:
            bad.append(f"{rel.as_posix()}: {msg}")
    for line in bad:
        print(line)
    if bad:
        print(f"{len(bad)} file(s) without a correct SPDX header (docs/licensing.md)")
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
