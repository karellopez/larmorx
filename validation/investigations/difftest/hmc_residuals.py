# SPDX-License-Identifier: Apache-2.0
"""How well mcflirt and larmorx's clean-room head-motion correction align the random phantoms
where they differ (the night's ``mri-hmc`` findings): for each finding, its shrunk inputs
rebuilt, FSL 6.0.7's mcflirt and ``larmorx mri hmc`` (in ``reproduce``, as the night compared,
and by default) run on its arguments, and the corrected series' temporal standard deviation in
the phantom (voxels of the corrected mean above 10 % of its maximum; lower is better aligned),
beside the uncorrected input's. Black-box: only the programs' outputs are read.

    python -P validation/investigations/difftest/hmc_residuals.py <night> <dir> [--max N]

Prints one line per finding and a summary (in how many cases larmorx's residual is lower,
equal within 1 %, or higher than mcflirt's; the median ratio).
"""

from __future__ import annotations

import json
import os
import shutil
import sys
from pathlib import Path

import nibabel as nib
import numpy as np
from larmorx_validation.difftest import core, inputs
from larmorx_validation.parity import defects, mri_hmc
from larmorx_validation.parity import fsl_mcflirt_replica as fr


def tsd(path: Path, mask_from: Path | None = None) -> float:
    x = np.asarray(nib.load(path).dataobj, dtype=np.float64)
    if x.ndim != 4 or x.shape[3] < 2:
        return float("nan")
    m = np.asarray(nib.load(mask_from).dataobj, dtype=np.float64) if mask_from else x
    mean = m.mean(axis=3) if m.ndim == 4 else m
    mask = mean > 0.1 * mean.max() if mean.max() > 0 else np.ones(mean.shape, bool)
    return float(x.std(axis=3)[mask].mean())


def main() -> int:
    argv = sys.argv[1:]
    most = None
    if "--max" in argv:
        i = argv.index("--max")
        most = int(argv[i + 1])
        del argv[i : i + 2]
    night, work = Path(argv[0]), Path(argv[1]).resolve()
    os.environ.setdefault("LARMORX_ORACLE_CACHE_MODE", "off")
    files = sorted((night / "findings" / "mri-hmc").glob("*-original.json"))[:most]
    ratios, better, same, worse = [], 0, 0, 0
    for p in files:
        finding = json.loads(p.read_text(encoding="utf-8"))
        recipe = finding.get("shrunk") or finding["recipe"]
        case = work / p.stem
        if case.exists():
            shutil.rmtree(case)
        case.mkdir(parents=True)
        paths = {}
        for name, spec in recipe["inputs"].items():
            q = case / (name + inputs.suffix_of(spec))
            inputs.materialize(spec, q)
            paths[name] = str(q)
        line = " ".join(core.substitute(core.arguments(recipe), paths.__getitem__))
        out = {}
        for who in ("mcflirt", "reproduce", "default"):
            d = case / who
            d.mkdir()
            args = line.replace("{run}", str(d)).split()
            if who == "mcflirt":
                code = fr.run_oracle(args, cwd=d, settings={}).code
            else:
                mode = defects.REPRODUCE if who == "reproduce" else defects.FIX
                code = mri_hmc._run_larmorx(args, {"FSLOUTPUTTYPE": "NIFTI_GZ"}, mode)[0]
            f = d / "out_mcf.nii.gz"
            out[who] = (
                tsd(f, case / "mcflirt" / "out_mcf.nii.gz") if code == 0 and f.is_file() else None
            )
        before = tsd(Path(paths["in"]))
        a, b = out["mcflirt"], out["reproduce"]
        if a and b:
            r = b / a
            ratios.append(r)
            better += r < 0.99
            same += 0.99 <= r <= 1.01
            worse += r > 1.01
        print(
            f"{p.stem}: input {before:.3f}; mcflirt {a}; larmorx reproduce {b}; default {out['default']}"
            f" | {' '.join(core.arguments(recipe))[:160]}",
            flush=True,
        )
    if ratios:
        print(
            f"SUMMARY {len(ratios)} cases: larmorx lower {better}, within 1 % {same}, higher {worse};"
            f" median ratio {float(np.median(ratios)):.3f}, quartiles"
            f" {float(np.quantile(ratios, 0.25)):.3f} {float(np.quantile(ratios, 0.75)):.3f}"
        )
    return 0


if __name__ == "__main__":
    sys.exit(main())
