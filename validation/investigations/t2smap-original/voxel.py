# SPDX-License-Identifier: Apache-2.0
"""One fit voxel of a ``t2smap`` run in several output folders (``specs/t2smap.md`` §7.3, §17.3)::

    python -I voxel.py <case folder> <index> <run name>...

``<case folder>`` holds the runs (``<case>/<run>/out``, as ``fmriprep_calls.py`` and ``compare.py``
leave them); ``<index>`` is the voxel's position among the mask's voxels (C order: ``check.py``'s
indices). For each run: the written full T2* (ms) and S0, the failure flag, and §17.3's objective
F = Σ (S0 · exp(−TE / T2*) − y)² over the voxel's samples of its fit; with the log-linear start
(the spec's model) and the voxel's count. Reads the inputs again with the spec's model.
"""

from __future__ import annotations

import sys
from pathlib import Path

import numpy as np

SPEC = Path(__file__).resolve().parent.parent / "t2smap-spec"
sys.path.insert(0, str(SPEC))
import check  # noqa: E402
import model  # noqa: E402


def main(argv: list[str]) -> int:
    case, index, runs = Path(argv[0]), int(argv[1]), argv[2:]
    ref = case / runs[0] / "out"
    words = check.call_words(ref)
    a = model.parser().parse_args(words)
    cwd = case / runs[0]
    a.data = [str((cwd / p).resolve()) if not Path(p).is_absolute() else p for p in a.data]
    if a.mask and not Path(a.mask).is_absolute():
        a.mask = str((cwd / a.mask).resolve())
    c = model.prepare(a)
    x_used, count, tes, mask = c["x_used"], c["count"], c["tes"], c["mask"]
    idx = np.flatnonzero(mask.ravel(order="C"))
    n = max(int(count[index]), 2)
    y = x_used[index, :n, :].astype(np.float64)
    te = np.repeat(tes[:n], y.shape[1])
    yv = y.reshape(-1)
    print(
        f"voxel {index} (grid {np.unravel_index(idx[index], mask.shape)}), count {count[index]}, "
        f"{n} echoes x {y.shape[1]} samples, min {yv.min()!r}, per-echo means {y.mean(axis=1)}"
    )
    for n_g, members in model.groups_of(count):
        if index in members:
            j = int(np.flatnonzero(members == index)[0])
            t2, s0 = model.loglin_group(x_used[members, :n_g, :], tes, "exact")
            print(f"log-linear start: S0 {s0[j]!r}, T2* {t2[j]!r} ms")
    for r in runs:
        out = case / r / "out"
        name = lambda d: model.NAMES[d][0] + (".tsv" if "tsv" in d else ".nii.gz")  # noqa: E731
        vals = {}
        for d in ("t2star img", "s0 img", "fit failures img", "limited t2star img"):
            img = check.load(out / name(d))[1]
            vals[d] = img.ravel(order="C")[idx][index] if img.ndim == 3 else None
        t2 = float(vals["t2star img"]) * 1000
        s0 = float(vals["s0 img"])
        f = check.objective(yv, te, s0, t2)
        print(
            f"{r:24s} T2* {t2!r} ms, S0 {s0!r}, limited T2* {float(vals['limited t2star img']) * 1000!r}, "
            f"failed {int(vals['fit failures img'])}, F {f!r}"
        )
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
