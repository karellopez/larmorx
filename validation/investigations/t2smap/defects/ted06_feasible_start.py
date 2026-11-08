# SPDX-License-Identifier: Apache-2.0
"""TED-06 (rule D): what a feasible start gives where tedana's curve fit rejects its start.

tedana's ``fit_monoexponential`` starts SciPy's ``curve_fit`` at the log-linear estimate with
the bounds ``((min(data), 0), (inf, inf))``; a start outside them (S0 below the voxel's smallest
sample, or T2* <= 0) is SciPy's ``ValueError: x0 is infeasible``, which tedana counts as a fit
failure, keeping the log-linear estimate. On fMRIPrep's multi-echo calls (the oracle runs'
``t2smap_node`` commands) this measures, with tedana 26.0.3's own functions: how many failures
are infeasible starts and why; and, for each, ``curve_fit`` from the start clipped into the
bounds (S0 raised to the smallest sample; a T2* <= 0 replaced by the mean echo time): whether it
converges, and the residual sum of squares of its result against the log-linear estimate's.

Usage: ``OPENBLAS_CORETYPE=Haswell scripts/orchestration/boxed.sh 16 <env>/bin/python -P
validation/investigations/t2smap/defects/ted06_feasible_start.py``
"""

from __future__ import annotations

import importlib.util
import sys
from pathlib import Path

import numpy as np
import scipy.optimize

HERE = Path(__file__).resolve().parent
_spec = importlib.util.spec_from_file_location(
    "check_stages", HERE.parent / "replica" / "check_stages.py"
)
stages = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(stages)

from tedana import decay, io, utils  # noqa: E402


def rss(s0, t2, tes1d, y):
    return float(np.sum((y - s0 * np.exp(-tes1d / t2)) ** 2))


def measure(name, args):
    d = args[args.index("-d") + 1 : args.index("-e")]
    rest = args[args.index("-e") + 1 :]
    tes = []
    for a in rest:
        if a.startswith("-"):
            break
        tes.append(float(a))
    tes = utils.check_te_values(tes)
    mask_path = args[args.index("--mask") + 1]
    ref = io.load_ref_img(d, len(tes))
    mask_img, _ = utils.load_mask(ref, mask=mask_path)
    data = io.load_data_nilearn(d, mask_img=mask_img, n_echos=len(tes))
    keep, am = utils.make_adaptive_mask(data, threshold=1, methods=["dropout"])
    data, am = data[keep], am[keep]
    t2_ll, s0_ll = decay.fit_loglinear(data, tes, am, report=False)
    tes = np.asarray(tes)
    n_vols = data.shape[2]
    out = {
        "voxels": len(am),
        "failures": 0,
        "infeasible_s0": 0,
        "infeasible_t2": 0,
        "other": 0,
        "refit_converged": 0,
        "refit_lower_rss": 0,
        "refit_failed": 0,
        "t2_ratio": [],
    }
    groups = sorted(set(am.tolist()) | ({2} if 1 in am else set()))
    for n in [g for g in groups if g >= 2]:
        idx = np.where(am <= 2)[0] if n == 2 else np.where(am == n)[0]
        tes1d = np.repeat(tes[:n], n_vols)
        for v in idx:
            y = data[v, :n, :].reshape(-1)
            lb = float(np.min(y))
            p0 = (s0_ll[v], t2_ll[v])
            r = decay._fit_single_voxel(v, tes1d, y, p0[0], p0[1], ((lb, 0), (np.inf, np.inf)))
            if not r[3]:
                continue
            out["failures"] += 1
            bad_s0, bad_t2 = not (p0[0] >= lb), not (p0[1] >= 0)
            if not (bad_s0 or bad_t2):
                out["other"] += 1
                continue
            out["infeasible_s0"] += bad_s0
            out["infeasible_t2"] += bad_t2
            start = (max(p0[0], lb), p0[1] if p0[1] > 0 else float(np.mean(tes[:n])))
            try:
                popt, _ = scipy.optimize.curve_fit(
                    decay.monoexponential, tes1d, y, p0=start, bounds=((lb, 0), (np.inf, np.inf))
                )
            except (RuntimeError, ValueError):
                out["refit_failed"] += 1
                continue
            out["refit_converged"] += 1
            if rss(popt[0], popt[1], tes1d, y) < rss(p0[0], p0[1], tes1d, y):
                out["refit_lower_rss"] += 1
            if p0[1] > 0:
                out["t2_ratio"].append(popt[1] / p0[1])
    q = np.percentile(out["t2_ratio"], [5, 50, 95]) if out["t2_ratio"] else []
    out["t2_ratio"] = [round(float(x), 3) for x in q]
    print(name, out, flush=True)
    return out


def main() -> int:
    for name, args in stages.real_cases():
        if name.endswith("-curvefit"):
            measure(name, args)
    return 0


if __name__ == "__main__":
    sys.exit(main())
