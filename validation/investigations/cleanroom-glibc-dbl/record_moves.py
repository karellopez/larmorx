# SPDX-License-Identifier: Apache-2.0
"""How far the iterative fits' results move with the C library's last bits alone.

Relay 2c's records (glibc's maths in ``reproduce``, the user's decision of 2026-11-04) moved some
registrations away from FSL's and most toward it. This probe runs a case of the clean-room
suites as the suite does, and also measures, for the same call:

- ``fsl_rep``: FSL's result against larmorx's ``reproduce`` result (glibc's functions), the
  record's own number;
- ``fsl_fix``: FSL's against larmorx's default (correctly rounded functions);
- ``rep_fix``: ``reproduce`` against the default: the same program, the same arithmetic, only
  the last bits of the C library's functions differ, so this is how sensitive the fit is to
  them;
- FLIRT only: ``band``, FSL's own variability (its own result on the input with uniform noise
  of +-0.5, the suite's band), and the suite's threshold.

FLIRT: the 95th-percentile displacement over the reference's brain (mm, the suite's measure).
mcflirt (``larmorx mri hmc``): the per-volume RMS deviation (radius 80 mm), its 95th percentile
over the volumes (mm, the suite's measure).

Usage (the worktree's ``python/`` and ``validation/src`` on ``PYTHONPATH``, the oracle cache in
``use`` mode)::

    python -P record_moves.py flirt <case> [<case> ...]
    python -P record_moves.py hmc <case> [<case> ...]

Prints one JSON line per case.
"""

from __future__ import annotations

import json
import sys
import tempfile
from pathlib import Path

import numpy as np
from larmorx_validation.parity import defects
from larmorx_validation.parity.harness import CheckList, Outcome


def _flirt(names: list[str]) -> None:
    from larmorx_validation.parity import fsl_flirt as fl

    by_name = {c.id: c for c in fl.cases("standard")}
    captured: dict[str, object] = {}
    real_fix_pass, real_band = fl.fix_pass, fl._band

    def band_spy(run, args, settings, a):
        captured["band"] = real_band(run, args, settings, a)
        captured["fsl_dir"] = a
        captured["args"] = args
        return captured["band"]

    def fix_spy(checks, program, prepare, args, settings, reproduced_dir, reproduced):
        with tempfile.TemporaryDirectory(prefix="lx-moves-fix-") as t_:
            t = Path(t_)
            if prepare is not None:
                prepare(t)
            fl.run_original(program, args, cwd=t, settings=settings, mode=defects.FIX)
            fsl_dir = captured["fsl_dir"]
            ref = fl._reference_path(args, fsl_dir)
            pts = fl._reference_mask(ref)[0] if ref is not None else None
            rows = {}
            for mat in sorted(p.name for p in Path(reproduced_dir).iterdir()):
                if not mat.endswith(".mat") or pts is None:
                    continue
                f, r, d = fsl_dir / mat, Path(reproduced_dir) / mat, t / mat
                if not (f.is_file() and r.is_file() and d.is_file()):
                    continue
                mf, mr, md = np.loadtxt(f), np.loadtxt(r), np.loadtxt(d)
                band = captured["band"](mat, pts) if captured.get("band") else None
                rows[mat] = {
                    "threshold": fl._threshold(args),
                    "fsl_rep": round(fl.displacement(mf, mr, pts)[0], 4),
                    "fsl_fix": round(fl.displacement(mf, md, pts)[0], 4),
                    "rep_fix": round(fl.displacement(mr, md, pts)[0], 4),
                    "band": None if band is None else round(band, 4),
                    "rep_identical_text": f.read_text() == r.read_text(),
                    "fix_identical_text": f.read_text() == d.read_text(),
                }
            captured["rows"] = rows
        return real_fix_pass(checks, program, prepare, args, settings, reproduced_dir, reproduced)

    fl.fix_pass, fl._band = fix_spy, band_spy
    for name in names:
        captured.clear()
        checks = CheckList()
        status = "pass"
        try:
            fl.run_case(by_name[name], checks)
        except Outcome as o:
            status = o.status
        if not all(c.passed for c in checks.checks):
            status = "fail"
        print(
            json.dumps(
                {
                    "suite": "fsl-flirt",
                    "case": name,
                    "status": status,
                    "matrices": captured.get("rows"),
                }
            ),
            flush=True,
        )


def _hmc(names: list[str]) -> None:
    from larmorx_validation.parity import mri_hmc as hm

    by_name = {c.id: c for c in hm.cases("standard")}
    captured: dict[str, object] = {}
    real_fix_pass = hm._fix_pass

    def p95(dir_a: Path, dir_b: Path, centre: np.ndarray) -> float:
        ma, _ = hm._read_mats(dir_a)
        mb, _ = hm._read_mats(dir_b)
        d = [hm.rms_deviation(x, y, centre) for x, y in zip(ma, mb, strict=True)]
        return round(float(np.percentile(d, 95)), 4)

    def fix_spy(checks, replay, reproduced, copied, reference, logs, histogram=False):
        with tempfile.TemporaryDirectory(prefix="lx-moves-hmc-") as t_:
            t = Path(t_)
            replay(t, defects.FIX)
            centre = hm._fov_centre(reference)
            rdir = hm.oracle_dir() / "runs" / captured["name"]
            rows = {}
            for key, path in reproduced.items():
                if not key.endswith(".mat") or not path.is_dir():
                    continue
                fsl, fix = rdir / key, t / key
                if fsl.is_dir() and fix.is_dir():
                    rows[key] = {
                        "fsl_rep": p95(fsl, path, centre),
                        "fsl_fix": p95(fsl, fix, centre),
                        "rep_fix": p95(path, fix, centre),
                        "thresholds (p95 RMS mm, median mm, median deg, FD r, source)": list(
                            hm._thresholds(captured["name"])
                        ),
                    }
            captured["rows"] = rows
        return real_fix_pass(checks, replay, reproduced, copied, reference, logs, histogram)

    hm._fix_pass = fix_spy
    for name in names:
        captured.clear()
        captured["name"] = name
        checks = CheckList()
        status = "pass"
        try:
            hm.run_case(by_name[name], checks)
        except Outcome as o:
            status = o.status
        if not all(c.passed for c in checks.checks):
            status = "fail"
        print(
            json.dumps(
                {
                    "suite": "mri-hmc",
                    "case": name,
                    "status": status,
                    "matrices": captured.get("rows"),
                }
            ),
            flush=True,
        )


if __name__ == "__main__":
    {"flirt": _flirt, "hmc": _hmc}[sys.argv[1]](sys.argv[2:])
