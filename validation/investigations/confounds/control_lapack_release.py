# SPDX-License-Identifier: Apache-2.0
"""A negative control for ``check_lapack.py``: larmorx told the other LAPACK release than the
running numpy's library must differ on the cases whose routines differ between the two trees
(``DORG2R``/``DORGL2``/``DORM2R``/``DORML2``/``DGEBD2``: ``DLARF`` with the stored 1 against
``DLARF1F``; ``DORMQR``/``DORMLQ``: the workspace and its short-workspace ``NB`` rule), so the
check's cases do tell the releases apart.

Usage (as ``check_lapack.py``, in one kept environment; every case defined):
``OPENBLAS_CORETYPE=Haswell OPENBLAS_NUM_THREADS=1 PYTHONPATH=<repo>/python python -P
control_lapack_release.py [--sections org,orm,brd]`` (quick tier; illegal arguments skipped).
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

sys.path.append(str(Path(__file__).resolve().parent))
import check_lapack as cl


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--sections", default="org,orm,brd")
    ap.add_argument("--threads", type=int, default=4)
    args = ap.parse_args()
    from larmorx import _core

    true = cl.LAPACK
    wrong = "3.12.1" if true == "3.12.0" else "3.12.0"
    print(f"library LAPACK {true}; larmorx told {wrong}")
    for name in args.sections.split(","):
        _, cases, run = cl.SECTIONS[name]
        tally: dict[str, list[int]] = {}
        for cid, case in enumerate(cases(True)):
            if "error" in case:
                continue
            cl.LAPACK = wrong  # larmorx's calls (and its own factorizations of the inputs)
            try:
                results = run(cid, case, args.threads, _core)
            finally:
                cl.LAPACK = true
            for ok, tag in results:
                t = tally.setdefault(tag, [0, 0])
                t[0] += 1
                t[1] += not ok
        for tag, (n, bad) in tally.items():
            print(f"{name} {tag}: {bad} / {n} differ")
    return 0


if __name__ == "__main__":
    sys.exit(main())
