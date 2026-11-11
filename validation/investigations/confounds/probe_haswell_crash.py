# SPDX-License-Identifier: Apache-2.0
"""Which numpy products crash numpy's own OpenBLAS (the wheel's ``libscipy_openblas64_``) when
its Haswell kernels are forced (``OPENBLAS_CORETYPE=Haswell``) on this machine, and whether the
CPU's own core (no override) or more threads change it. Each product runs in a process of its
own (a crash is its exit status, ``-11`` for a segmentation fault).

``--l2-size N`` sets ``OPENBLAS_L2_SIZE`` (KiB) in the children: OpenBLAS >= 0.3.31 raises
``GEMM_Q`` to 512 only when the L2 size it reads (this variable first, else cpuid) is 1024 KiB
on an AMD AVX-512 CPU (``kernel/setparam-ref.c``), so any other value keeps the Haswell table's
256 on this machine (relay 2b-i: no shape crashes then). ``--one`` runs only the forced Haswell
core at one thread.

Usage: ``python -I probe_haswell_crash.py [--python <interpreter> ...] [--l2-size N] [--one]``
(default: this interpreter). Prints one line per (interpreter, core, threads) with the shapes
that crashed.
"""

from __future__ import annotations

import argparse
import itertools
import os
import subprocess
import sys

CHILD = r"""
import sys, numpy as np
m, n, p, order = int(sys.argv[1]), int(sys.argv[2]), int(sys.argv[3]), sys.argv[4]
a = np.ones((m, n), order=order); b = np.ones((n, p), order=order)
c = a @ b
assert c.shape == (m, p) and float(c[0, 0]) == n
"""

SHAPES = sorted(
    set(itertools.product([1, 2, 4, 5, 25, 515], [255, 256, 257, 511, 512, 513], [8, 24, 25, 515]))
)


def run(py: str, core: str | None, threads: str, shape, order: str) -> int:
    env = {k: v for k, v in os.environ.items() if k not in ("OPENBLAS_CORETYPE",)}
    env["OPENBLAS_NUM_THREADS"] = threads
    if core:
        env["OPENBLAS_CORETYPE"] = core
    args = [py, "-I", "-c", CHILD, *map(str, shape), order]
    return subprocess.run(args, env=env, capture_output=True, timeout=120).returncode


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--python", nargs="*", default=[sys.executable])
    ap.add_argument("--orders", default="C,F")
    ap.add_argument("--l2-size", type=int, help="OPENBLAS_L2_SIZE for the children (KiB)")
    ap.add_argument("--one", action="store_true", help="only the Haswell core at one thread")
    args = ap.parse_args()
    if args.l2_size is not None:
        os.environ["OPENBLAS_L2_SIZE"] = str(args.l2_size)
    configs = [("Haswell", "1"), ("Haswell", "8"), (None, "1")]
    for py in args.python:
        for core, threads in configs[:1] if args.one else configs:
            crashed = []
            for shape in SHAPES:
                for order in args.orders.split(","):
                    rc = run(py, core, threads, shape, order)
                    if rc != 0:
                        crashed.append((shape, order, rc))
            label = f"{py} core={core or 'own'} threads={threads} l2={args.l2_size or 'cpuid'}"
            print(
                f"{label}: {len(crashed)} of {len(SHAPES) * len(args.orders.split(','))} crashed",
                flush=True,
            )
            for s in crashed[:40]:
                print("   ", s, flush=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
