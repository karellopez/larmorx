# SPDX-License-Identifier: Apache-2.0
"""TED-46's saturated voxels, value by value (relay C4, 2026-11-12): the suite's ``x-ted46-*``
cases run by tedana 26.0.3 (through the oracle-output cache), the clean-room original in
``reproduce`` and in its default, and the replica's default (``LARMORX_GPL_BIN``, a black box);
for the block of 18 voxels the suite's inputs saturate at 255 (``compare.ted46_inputs``), every
map's values compared: (A) tedana against ``reproduce`` (the exact ports of numpy's ``lstsq`` and
SciPy's ``curve_fit`` there: expected identical), (B) the replica's default against the original's
(the exact solutions: S0 255, T2* +inf floored to 500 ms, variances 0 or +inf).

    PYTHONPATH=<worktree>/python:<worktree>/validation/src python -P ted46_values.py <work>
"""

from __future__ import annotations

import sys
from pathlib import Path

import nibabel as nib
import numpy as np

MAPS = (
    "T2starmap.nii.gz",
    "S0map.nii.gz",
    "desc-limited_T2starmap.nii.gz",
    "desc-limited_S0map.nii.gz",
    "desc-decayFitFailures_mask.nii.gz",
    "stat-variance_desc-s0_statmap.nii.gz",
    "stat-variance_desc-t2star_statmap.nii.gz",
    "desc-rmse_statmap.nii.gz",
)


def block(path: Path) -> np.ndarray | None:
    if not path.exists():
        return None
    a = np.asanyarray(nib.load(str(path)).dataobj)
    return a[9:12, 5:8, 2:4, ...].reshape(-1)


def same(a: np.ndarray, b: np.ndarray) -> str:
    eq = (a == b) | (np.isnan(a) & np.isnan(b))
    if eq.all():
        return f"{a.size} identical"
    d = np.abs(a.astype(np.float64) - b) / np.maximum(np.abs(a), np.abs(b))
    return f"{int(eq.sum())}/{a.size} identical, largest relative {np.nanmax(d[~eq]):.3g}"


def main(argv: list[str]) -> int:
    from larmorx_validation.parity import tedana_t2smap_original as so

    work = Path(argv[0]).resolve()
    blackbox, compare, _, _ = so.tools()
    inputs = so.inputs_dir()
    env = blackbox.environment()
    for name, args, _ in compare.EXTRA:
        if not name.startswith("x-ted46"):
            continue
        full = [w.replace("<I>", str(inputs)) for w in args] + ["--out-dir", "out"]
        d = {k: work / name / k for k in ("tedana", "reproduce", "default", "replica")}
        so.run_tedana(full, d["tedana"], env)
        so.run_original(full, d["reproduce"], {**env, "LARMORX_UPSTREAM_DEFECTS": "reproduce"})
        so.run_original(full, d["default"], {**env, "LARMORX_UPSTREAM_DEFECTS": "fix"})
        if so.replica_program():
            so.run_replica(full, d["replica"], env)
        print(f"== {name}")
        for m in MAPS:
            t, r = block(d["tedana"] / "out" / m), block(d["reproduce"] / "out" / m)
            o, p = block(d["default"] / "out" / m), block(d["replica"] / "out" / m)
            if t is None and o is None:
                continue
            line = f"  {m}: (A) {same(t, r) if t is not None else '-'}"
            line += f"; (B) {same(p, o) if p is not None else '-'}"
            if o is not None and m in ("T2starmap.nii.gz", "S0map.nii.gz"):
                line += f"; default {np.unique(o)[:4]}"
            if t is not None and m == "T2starmap.nii.gz":
                line += f"; tedana {np.unique(t)[:4]}"
            print(line)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
