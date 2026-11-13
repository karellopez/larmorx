# SPDX-License-Identifier: Apache-2.0
"""Re-made records against the committed ones, every case and every reported field (the orchestration's
``compare_records.py`` shows the first five; its stripping of times, temporary paths, addresses and
versions is reused), with the cases and checks counted.

Usage: python -P compare_remade_records.py <dir> <name>...   (run from the checkout: the re-made record
of <name> is the one JSON in <dir>/<name>/, the committed one docs/validation/<name>.json). Exit status 1
when a case of the committed record is gone or changed its status, or a new case failed.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "scripts" / "orchestration"))
from compare_records import strip  # noqa: E402


def _short(x: object, n: int) -> str:
    s = str(x)
    return s if len(s) <= n else s[:n] + "..."


def main(argv: list[str]) -> int:
    rerun, names = Path(argv[1]), argv[2:]
    bad = 0
    for name in names:
        (f,) = sorted((rerun / name).glob("*.json"))
        a = json.loads(f.read_text(encoding="utf-8"))
        b = json.loads((ROOT / "docs" / "validation" / f"{name}.json").read_text(encoding="utf-8"))
        A = {r["case"]: r for r in a["results"]}
        B = {r["case"]: r for r in b["results"]}
        checks = [sum(r.get("checks_total", 0) for r in R.values()) for R in (A, B)]
        same = [c for c in A if c in B and strip(A[c]) == strip(B[c])]
        print(
            f"== {name}: re-made {a['environment'].get('larmorx_commit')} {len(A)} cases, {checks[0]} "
            f"checks | committed {b['environment'].get('larmorx_commit')} {len(B)} cases, {checks[1]} "
            f"checks; {len(same)} of the committed cases identical"
        )
        for c in sorted(set(A) | set(B)):
            if c not in B:
                print(f"   NEW {c}: {A[c]['status']}")
                bad |= A[c]["status"] == "fail"
                continue
            if c not in A:
                print(f"   GONE {c}: {B[c]['status']}")
                bad = 1
                continue
            ra, rb = strip(A[c]), strip(B[c])
            if ra == rb:
                continue
            print(f"   CASE {c}: {A[c]['status']} | committed {B[c]['status']}")
            bad |= A[c]["status"] != B[c]["status"]
            for k in sorted(set(ra) | set(rb)):
                va, vb = ra.get(k), rb.get(k)
                if va == vb:
                    continue
                if isinstance(va, dict) and isinstance(vb, dict):
                    for sk in sorted(set(va) | set(vb)):
                        if va.get(sk) != vb.get(sk):
                            print(f"      {k}[{_short(sk, 160)}]: {_short(va.get(sk), 300)}")
                            print(f"         committed: {_short(vb.get(sk), 300)}")
                else:
                    print(f"      {k}: {_short(va, 600)}\n         committed: {_short(vb, 600)}")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
