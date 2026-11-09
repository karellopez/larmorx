# SPDX-License-Identifier: Apache-2.0
"""Whether golden cases depend on the folder they run in (SDC-45, PIPE-37): every case of
``tests/golden/registry.py`` not yet in ``golden.json`` (or the cases named) runs below two
folders, one named as ``scripts/record_golden.py``'s temporary folder (``larmorx-golden-<x>``,
which holds pybids' unanchored ``den-`` pattern) and one not, and their fingerprints are
compared: a case whose outputs depend on the folders above its dataset would be recorded with
checksums no other machine reproduces.

    python -P golden_folders.py <scratch folder> [<case id> ...]

Prints each case's seconds, ``DIFFER <case>`` or ``ERROR <case>`` lines, and
``GOLDEN-FOLDERS-END <n> cases, <k> differ or fail``; exit status 1 when ``k > 0``.
"""

from __future__ import annotations

import json
import sys
import time
import warnings
from pathlib import Path

REPO = Path(__file__).resolve().parents[4]


def main() -> int:
    sys.path.insert(0, str(REPO / "tests" / "golden"))
    import registry

    scratch = Path(sys.argv[1])
    golden = json.loads((REPO / "tests/golden/golden.json").read_text(encoding="utf-8"))
    todo = sys.argv[2:] or sorted(c for c in registry.CASES if c not in golden["cases"])
    print(len(todo), "cases")
    out = []
    for name in ("plainrun", "larmorx-golden-abc"):
        d = scratch / name
        (d / "work").mkdir(parents=True, exist_ok=True)
        inp = registry.Inputs(d / "inputs")
        res = {}
        for cid in todo:
            ww = d / "work" / cid.replace("/", "_")
            ww.mkdir()
            t = time.time()
            with warnings.catch_warnings():
                warnings.simplefilter("ignore")
                try:
                    res[cid] = registry.fingerprints(
                        registry.CASES[cid].run(registry.Context(ww, inp))
                    )
                except Exception as err:
                    res[cid] = f"ERROR {type(err).__name__}: {err}"[:300]
            if name == "plainrun":
                print(f"  {cid} {time.time() - t:.1f} s", flush=True)
        out.append(res)
    a, b = out
    bad = 0
    for cid in todo:
        if isinstance(a[cid], str) or isinstance(b[cid], str):
            bad += 1
            print("ERROR", cid, a[cid] if isinstance(a[cid], str) else b[cid])
        elif a[cid] != b[cid]:
            bad += 1
            print("DIFFER", cid, [k for k in a[cid] if a[cid][k] != b[cid].get(k)][:10])
    print(f"GOLDEN-FOLDERS-END {len(todo)} cases, {bad} differ or fail")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
