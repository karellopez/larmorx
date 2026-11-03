# SPDX-License-Identifier: Apache-2.0
"""FSL-143 (rule A, measured): applytopup's ``--method=vb3D`` and ``vb4D`` smoothness prior.

1. The prior's precision matrix assembled from its blocks (the diagonal block and the couplings
   one and two lines away in the slice, one and two slices away, one line and one slice away)
   on a small grid, with FSL's coupling between neighbouring slices (half the squared
   second-difference operator's) and with the operator's own: the smallest eigenvalues.
2. The replica on the suite's ``at/vb3d-y`` and ``at/vb4d-y`` inputs (the half-resolution pair,
   topup's own field) in ``reproduce`` mode (FSL 6.0.7.23's applytopup bit for bit: the suite's
   cases) and in the default ``fix`` mode: the range of the restored image beside the input's.

    python -P vb_prior.py <larmorx-nc> <work dir>

with ``PYTHONPATH`` naming the worktree's ``validation/src`` (the suite prepares the case
directories: FSL's topup outputs come from the oracle cache, nothing of FSL runs here). Run it
inside a memory box (``scripts/orchestration/boxed.sh 8``).
"""

from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path

import nibabel as nib
import numpy as np


def blocks(sz: int, sf0: float, sf1: float, fixed: bool) -> dict[tuple[int, int], np.ndarray]:
    """The prior's blocks (``sz`` voxels along the phase encoding, circular), keyed by the
    offset (lines, slices) they couple."""

    def circ(vals: dict[int, float]) -> np.ndarray:
        m = np.zeros((sz, sz))
        for r in range(sz):
            for off, v in vals.items():
                m[r, (r + off) % sz] = v
        return m

    c = 2 + 2 * sf0 + 2 * sf1
    b = {
        (0, 0): circ(
            {
                0: c * c + 2 + 2 * sf0 * sf0 + 2 * sf1 * sf1,
                1: -2 * c,
                -1: -2 * c,
                2: 1.0,
                -2: 1.0,
            }
        ),
        (1, 0): circ({0: -2 * sf0 * c, 1: 2 * sf0, -1: 2 * sf0}),
        (2, 0): circ({0: sf0 * sf0}),
        (0, 1): circ(
            {0: -2 * sf1 * c, 1: (2 if fixed else 1) * sf1, -1: (2 if fixed else 1) * sf1}
        ),
        (0, 2): circ({0: sf1 * sf1}),
        (1, 1): circ({0: 2 * sf0 * sf1}),
    }
    return b


def precision(sz: int, nrow: int, nsl: int, sf0: float, sf1: float, fixed: bool) -> np.ndarray:
    """The precision matrix over every line (i) of every slice (k) as tau_update and mu_update
    couple them (lines and slices not circular)."""
    b = blocks(sz, sf0, sf1, fixed)
    n = nrow * nsl
    q = np.zeros((n * sz, n * sz))

    def at(k: int, i: int) -> slice:
        j = k * nrow + i
        return slice(j * sz, (j + 1) * sz)

    for k in range(nsl):
        for i in range(nrow):
            for (di, dk), m in b.items():
                for si in (di, -di) if di else (0,):
                    for sk in (dk, -dk) if dk else (0,):
                        i2, k2 = i + si, k + sk
                        if 0 <= i2 < nrow and 0 <= k2 < nsl:
                            q[at(k, i), at(k2, i2)] = m
    return q


def ranges(path: Path) -> str:
    a = np.asarray(nib.load(str(path)).dataobj, float)
    nz = a[a != 0]
    return f"min {a.min():.4g}, max {a.max():.4g}, mean of the nonzero {nz.mean():.4g}"


def main() -> int:
    exe, work = sys.argv[1], Path(sys.argv[2])
    print(
        "1. the prior's smallest eigenvalues (8 voxels along the phase encoding, 6 lines, 5 slices)"
    )
    for sf0, sf1 in ((1.0, 6 / 6.6), (1.5, 6 / 6.6), (1.0, 1.0)):
        for fixed in (False, True):
            q = precision(8, 6, 5, sf0, sf1, fixed)
            ev = np.linalg.eigvalsh((q + q.T) / 2)
            name = "the operator's (fix)" if fixed else "FSL's (reproduce)"
            print(
                f"   sf = ({sf0:.3g}, {sf1:.3g}) {name:22s} smallest {ev[0]:+.4g}, "
                f"negative {int((ev < -1e-9).sum())} of {len(ev)}"
            )
    from larmorx_validation.parity import fsl_topup_replica as t

    print("2. the replica on the suite's inputs (reproduce = FSL's applytopup bit for bit)")
    node = t._source_node()
    for case in ("at/vb3d-y", "at/vb4d-y"):
        run = t._all_cases()[case][2]
        for mode in ("reproduce", "fix"):
            d = work / case.replace("/", "-") / mode
            d.mkdir(parents=True, exist_ok=True)
            args = t._prepare(run, d, node)
            env = dict(os.environ, LARMORX_UPSTREAM_DEFECTS=mode)
            subprocess.run(
                [exe, "fsl", "applytopup", *args],
                cwd=d,
                env=env,
                check=True,
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
            )
            if mode == "reproduce":
                print(f"   {case} input   {ranges(d / 'half_up.nii.gz')}")
            print(f"   {case} {mode:9s} {ranges(d / 'hifi.nii.gz')}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
