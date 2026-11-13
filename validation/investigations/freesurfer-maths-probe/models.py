# SPDX-License-Identifier: Apache-2.0
"""Statements of ``specs/freesurfer-maths.md`` checked on the prebuilt probe as models of its printed
numbers (black box: the probe's lines and the spec's text, nothing else).

    python -P models.py affine --probe P --scratch DIR
        §3's operand order: every ``affine`` line of the group ``affine`` (its seeded batch and the
        fixed batch ``affine-nan``) against AffineMV and AffineMM computed in float with x86-64's NaN
        rules (§7) in the order the spec gives: in each product the vector's number (for AffineMM the
        second matrix's number) is the left operand, in each sum the running sum

The probe runs through the oracle-output cache (one thread, the family ``freesurfer``) when the
validation package is importable (``PYTHONPATH=validation/src``), else directly. Exit status 1 when a
line differs from its model.
"""

from __future__ import annotations

import argparse
import importlib.util
import sys
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent


def _compare():
    spec = importlib.util.spec_from_file_location("fs_maths_compare", HERE / "compare.py")
    mod = importlib.util.module_from_spec(spec)
    sys.modules["fs_maths_compare"] = mod
    spec.loader.exec_module(mod)
    return mod


C = _compare()

DEFAULT_NAN = 0x7FC00000 | 0x80000000  # a NaN made from numbers: SSE's, the sign bit set


def isnan(b: int) -> bool:
    return (b & 0x7F800000) == 0x7F800000 and (b & 0x007FFFFF) != 0


def f32(b: int) -> np.float32:
    return np.array([b], dtype=np.uint32).view(np.float32)[0]


def sse(op, a: int, b: int) -> int:
    """One float operation with x86-64's NaN rules (§7): a NaN operand passes on quieted, the left
    one of two; a NaN made from numbers is the default NaN (the sign bit set)."""
    if isnan(a):
        return a | 0x00400000
    if isnan(b):
        return b | 0x00400000
    with np.errstate(all="ignore"):
        r = int(np.array([op(f32(a), f32(b))], dtype=np.float32).view(np.uint32)[0])
    return DEFAULT_NAN if isnan(r) else r


def fmul(a: int, b: int) -> int:
    return sse(lambda x, y: x * y, a, b)


def fadd(a: int, b: int) -> int:
    return sse(lambda x, y: x + y, a, b)


def to_affine(row_major: list[int]) -> list[int]:
    """§3: float i + 4j is the matrix's number at row-major place j + 4i."""
    return [row_major[(k // 4) + 4 * (k % 4)] for k in range(16)]


def column(a: list[int], v: list[int]) -> list[int]:
    out = []
    for i in range(4):
        acc = fmul(v[0], a[i])
        for j in (1, 2, 3):
            acc = fadd(acc, fmul(v[j], a[i + 4 * j]))
        out.append(acc)
    return out


def affine(runner, scratch: Path) -> int:
    batches = C.Gen(7).affine()
    good = bad = 0
    for b in batches:
        path = C.write_batch(scratch, b)
        out = C.parse(
            runner._run(
                [str(runner.probe), "batch", str(path)],
                dict(C.BASE_ENV),
                "",
                "freesurfer-maths-probe",
            )
        )
        for src, line in zip(b.lines, out, strict=True):
            tok = src.split()
            if tok[0] != "affine":
                continue
            a = to_affine([int(t, 16) for t in tok[4:20]])
            m2 = to_affine([int(t, 16) for t in tok[23:39]])
            vec = [int(t, 16) for t in tok[39:43]]
            res = line.split(b" | ")[0].decode().split()
            w = [int(t, 16) for t in res[4:8]]
            mm = [int(t, 16) for t in res[14:30]]
            want_mm = [x for j in range(4) for x in column(a, m2[4 * j : 4 * j + 4])]
            ok = w == column(a, vec) and mm == want_mm
            good += ok
            bad += not ok
            if not ok and bad <= 3:
                print(f"DIFFERS: {src[:120]}\n    probe {line[:200]!r}")
    print(f"affine: {good + bad} affine lines: as §3's order {good}, not {bad}")
    return 1 if bad else 0


def main() -> int:
    ap = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    ap.add_argument("command", choices=["affine"])
    ap.add_argument("--probe", type=Path, required=True)
    ap.add_argument("--scratch", type=Path, required=True)
    ap.add_argument("--no-cache", action="store_true")
    a = ap.parse_args()
    scratch = a.scratch.resolve()
    scratch.mkdir(parents=True, exist_ok=True)
    runner = C.Runner(a.probe.resolve(), scratch, not a.no_cache)
    return affine(runner, scratch)


if __name__ == "__main__":
    sys.exit(main())
