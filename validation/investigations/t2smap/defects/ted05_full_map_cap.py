# SPDX-License-Identifier: Apache-2.0
"""TED-05 (rule D): what capping the full T2* map would change.

tedana 26.0.3 caps only the limited T2* map (values above ten times its 99.5th percentile set to
it); the full map, which fMRIPrep uses and from which the optimal combination is weighted, keeps
its largest values (tedana 25.0.1's t2smap capped the full map too, by its own percentile). With
tedana's own functions on fMRIPrep's multi-echo calls (curvefit), this counts the full-map
voxels above ten times the cap and how much the combined series changes when they are capped.

Usage: ``OPENBLAS_CORETYPE=Haswell scripts/orchestration/boxed.sh 16 <env>/bin/python -P
validation/investigations/t2smap/defects/ted05_full_map_cap.py``
"""

from __future__ import annotations

import importlib.util
import sys
from pathlib import Path

import numpy as np
from scipy import stats

HERE = Path(__file__).resolve().parent
_spec = importlib.util.spec_from_file_location(
    "check_stages", HERE.parent / "replica" / "check_stages.py"
)
stages = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(stages)

from tedana import combine, decay, io, utils  # noqa: E402


def measure(name, args):
    d = args[args.index("-d") + 1 : args.index("-e")]
    tes = []
    for a in args[args.index("-e") + 1 :]:
        if a.startswith("-"):
            break
        tes.append(float(a))
    tes = utils.check_te_values(tes)
    ref = io.load_ref_img(d, len(tes))
    mask_img, _ = utils.load_mask(ref, mask=args[args.index("--mask") + 1])
    data = io.load_data_nilearn(d, mask_img=mask_img, n_echos=len(tes))
    keep, am = utils.make_adaptive_mask(data, threshold=1, methods=["dropout"])
    t2s, s0, *_ = decay.fit_decay(data[keep], tes, am[keep], "curvefit", report=False)
    t2s, s0, t2s_lim, _ = decay.modify_t2s_s0_maps(t2s, s0, am[keep], tes)
    cap26 = stats.scoreatpercentile(t2s_lim.flatten(), 99.5, interpolation_method="lower")
    cap25 = stats.scoreatpercentile(t2s.flatten(), 99.5, interpolation_method="lower")
    full = utils.unmask(t2s, keep)
    oc = combine.make_optcom(data, tes, am, t2s=full, combmode="t2s")
    out = {"voxels": int(keep.sum()), "full_max_ms": float(t2s.max())}
    for label, cap in (("cap_limited", cap26), ("cap_full_25.0.1", cap25)):
        capped = full.copy()
        hit = capped > cap * 10
        capped[hit] = cap
        oc2 = combine.make_optcom(data, tes, am, t2s=capped, combmode="t2s")
        rel = np.abs(oc2 - oc) / np.maximum(np.abs(oc), 1e-12)
        out[label] = {
            "cap_ms": round(float(cap), 3),
            "voxels_capped": int(hit.sum()),
            "series_values_changed": int((oc2 != oc).sum()),
            "max_relative_change": float(rel.max()),
            "median_relative_change_where_changed": float(np.median(rel[oc2 != oc]))
            if (oc2 != oc).any()
            else 0.0,
        }
    print(name, out, flush=True)


def main() -> int:
    for name, args in stages.real_cases():
        if name.endswith("-curvefit"):
            measure(name, args)
    return 0


if __name__ == "__main__":
    sys.exit(main())
