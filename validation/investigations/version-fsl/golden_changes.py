# SPDX-License-Identifier: Apache-2.0
"""Which golden checksums of the larmorx-nc replicas changed between two versions of
``crates-nc/larmorx-nc-cli/tests/golden.tsv``: cases added, removed, and outputs whose checksum
changed (by case and output kind).

Usage: python -P golden_changes.py <old golden.tsv> <new golden.tsv>
"""

from __future__ import annotations

import sys
from collections import Counter
from pathlib import Path


def rows(path: Path) -> dict[tuple[str, str], str]:
    out = {}
    for line in path.read_text(encoding="utf-8").splitlines():
        if not line or line.startswith("#"):
            continue
        case, output, sha, *_ = line.split("\t")
        out[(case, output)] = sha
    return out


def main() -> int:
    old, new = rows(Path(sys.argv[1])), rows(Path(sys.argv[2]))
    oc, nc = {c for c, _ in old}, {c for c, _ in new}
    print(f"cases {len(oc)} -> {len(nc)}; outputs {len(old)} -> {len(new)}")
    print("added cases:", sorted(nc - oc))
    print("removed cases:", sorted(oc - nc))
    changed = [k for k in old.keys() & new.keys() if old[k] != new[k]]
    kinds = Counter(o.rsplit(".", 1)[-1] for _, o in changed)
    print(f"{len(changed)} outputs changed, by kind: {dict(kinds)}")
    print("cases with a changed output:", sorted({c for c, _ in changed}))
    return 0


if __name__ == "__main__":
    sys.exit(main())
