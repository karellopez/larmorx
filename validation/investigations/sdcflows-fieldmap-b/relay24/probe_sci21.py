# SPDX-License-Identifier: Apache-2.0
"""How often SCI-21's SciPy calls (the 1-D running rank writing past its output's end) return,
crash or hang in a process of their own, per case, under several ways of starting that
process: what makes check_ndimage's SCI-21 count deterministic (relay 24).

Usage: python -P probe_sci21.py --runs N --mode inherit|fixed|fixed-noaslr [--jobs J]
"""

from __future__ import annotations

import argparse
import collections
import os
import subprocess
import sys
from concurrent.futures import ThreadPoolExecutor

X = [3.0, 1, 4, 1, 5, 9, 2, 6, 5]
CASES = {
    "float64 into float32": ("float64", "np.float32"),
    "float64 into int16": ("float64", "np.int16"),
    "float64 into uint8": ("float64", "np.uint8"),
    "float32 into float64": ("float32", "np.float64"),
    "float32 into int64": ("float32", "np.int64"),
    "int64 into float64": ("int64", "np.float64"),
    "int64 into int32": ("int64", "np.int32"),
    "float64 into >f8": ("float64", "np.dtype('>f8')"),
    "float32 into >f4": ("float32", "np.dtype('>f4')"),
    "float64 into a float32 array": ("float64", "np.full(9, 7, np.float32)"),
    "float32 into a float64 array": ("float32", "np.full(9, 7.0)"),
    "float32 into a strided float64 array": ("float32", "np.full((9, 2), 7.0)[:, 0]"),
}
FNS = ("median_filter(x, 3", "rank_filter(x, 1, 3", "percentile_filter(x, 50, 3")


def code(src: str, out: str, fn: str) -> str:
    return (
        "import warnings, numpy as np\nfrom scipy import ndimage as sp\n"
        f"x = np.array({X}, dtype='{src}')\n"
        "with warnings.catch_warnings(record=True) as w:\n"
        "    warnings.simplefilter('always')\n"
        f"    r = sp.{fn}, output={out})\n"
        "print(r.dtype.str, np.ascontiguousarray(r).tobytes().hex(), "
        "repr([f'{type(m.message).__name__}: {m.message}' for m in w]), flush=True)\n"
    )


def one(py: str, mode: str, c: str) -> str:
    if mode == "inherit":
        cmd, env, cwd = [py, "-I", "-c", c], None, None
    else:
        keep = ("OPENBLAS_CORETYPE", "OPENBLAS_NUM_THREADS", "OMP_NUM_THREADS")
        env = {k: os.environ[k] for k in keep if k in os.environ}
        env["PYTHONHASHSEED"] = "0"
        env["LC_ALL"] = "C"
        cmd = [py, "-s", "-P", "-c", c]
        if mode == "fixed-noaslr":
            cmd = ["setarch", "x86_64", "-R", *cmd]
        cwd = "/"
    try:
        p = subprocess.run(cmd, capture_output=True, text=True, timeout=30, env=env, cwd=cwd)
    except subprocess.TimeoutExpired:
        return "hung"
    out = p.stdout.strip()
    return f"{'result' if out else 'none'} exit {p.returncode}" + (f" {hash(out) & 0xffff:04x}" if out else "")


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--runs", type=int, default=10)
    ap.add_argument("--mode", default="inherit")
    ap.add_argument("--jobs", type=int, default=8)
    args = ap.parse_args()
    py = sys.executable
    jobs = [(lab, fn) for lab in CASES for fn in FNS for _ in range(args.runs)]
    with ThreadPoolExecutor(args.jobs) as ex:
        outs = list(ex.map(lambda j: one(py, args.mode, code(*CASES[j[0]], j[1])), jobs))
    tally: dict[tuple, collections.Counter] = collections.defaultdict(collections.Counter)
    for (lab, fn), o in zip(jobs, outs, strict=True):
        tally[(lab, fn)][o] += 1
    varying = 0
    for k, c in tally.items():
        if len(c) > 1 or any(not o.startswith("result exit 0") for o in c):
            varying += len(c) > 1
            print(f"{k[0]} | {k[1]}: {dict(c)}")
    print(f"{args.mode}: {len(tally)} cases x {args.runs}; {varying} with more than one outcome")
    return 0


if __name__ == "__main__":
    sys.exit(main())
