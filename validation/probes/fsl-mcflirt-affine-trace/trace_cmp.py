# SPDX-License-Identifier: Apache-2.0
"""Compares mcflirt's cost trace (``run_call.py trace ... --preload libcost_trace.so``) with the
replica's debug lines (a build instrumented by ``debug_patch.py``, run with
``LARMORX_NC_DEBUG=<volume>:<scale>``) for one volume: each evaluation's affine and cost, as
bits, until the first difference.

Usage: ``python -P trace_cmp.py <mcflirt trace.txt> <replica stderr> <volume> <stages, e.g. 2,3>``
"""

import re
import struct
import sys


def parse_mcflirt(path: str, vol: int, stages: list[int]) -> list[tuple[int, str, str]]:
    stage = 0
    cur = None
    out = []
    with open(path, encoding="utf-8", errors="replace") as f:
        for line in f:
            if "iteration" in line:
                stage += 1
            for m in re.finditer(r"\[(\d+)\]", line):
                cur = int(m.group(1))
            if "DBG" in line and cur == vol and stage in stages:
                p = line[line.index("DBG") :].split()
                out.append((stage, p[3], p[4]))
    return out


def parse_replica(path: str) -> list[tuple[str, str]]:
    with open(path, encoding="utf-8") as f:
        return [(p[3], p[4]) for p in (ln.split() for ln in f if ln.startswith("DBG"))]


def mat(h: str) -> list[list[float]]:
    vals = [struct.unpack("<d", bytes.fromhex(x)[::-1])[0] for x in h.split(",")]
    return [[vals[c * 4 + r] for c in range(4)] for r in range(4)]


def main() -> int:
    mt, rp = sys.argv[1], sys.argv[2]
    vol, stages = int(sys.argv[3]), [int(s) for s in sys.argv[4].split(",")]
    a = parse_mcflirt(mt, vol, stages)
    b = parse_replica(rp)
    print(len(a), "mcflirt evaluations;", len(b), "replica")
    for i, ((st, ma, ca), (mb, cb)) in enumerate(zip(a, b, strict=False)):
        if ma != mb or ca != cb:
            print(f"first difference at {i}, stage {st}: matrix same {ma == mb}, cost {ca} vs {cb}")
            if ma != mb:
                for ra, rb in zip(mat(ma), mat(mb), strict=True):
                    print("   mcflirt", [f"{x:.17g}" for x in ra])
                    print("   replica", [f"{x:.17g}" for x in rb])
            return 1
    print("no difference in", min(len(a), len(b)))
    return 0


if __name__ == "__main__":
    sys.exit(main())
