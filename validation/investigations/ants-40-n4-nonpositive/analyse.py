# SPDX-License-Identifier: Apache-2.0
"""ANTS-40, second step: from ``measure.py``'s outputs, split each change of the bias field
into its global scale (N4's field has no fixed scale: the median of the ratio) and its shape
(the ratio over its median), and measure what the correction does to the corrected image's
uniformity where fMRIPrep's tissue maps are on the input's grid: the white matter's
coefficient of variation (fMRIPrep's WM probability > 0.9), lower being better.

Usage::

    PYTHONPATH=python:validation/src python -P validation/investigations/ants-40-n4-nonpositive/analyse.py --out <measure.py's out dir>

It writes ``<out>/analysis.json`` and prints a Markdown table.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

import nibabel as nib
import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent))
from measure import RUNS, arguments, brain_mask


def wm_mask(run: Path, img: nib.Nifti1Image) -> np.ndarray | None:
    for m in sorted((run / "out").rglob("*/anat/*label-WM_probseg.nii.gz")):
        if "space-" in m.name:
            continue
        mi = nib.load(m)
        if mi.shape[:3] == img.shape[:3] and np.allclose(mi.affine, img.affine, atol=1e-3):
            return np.asanyarray(mi.dataobj) > 0.9
    return None


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--out", type=Path, required=True)
    args = ap.parse_args()
    results = json.loads((args.out / "results.json").read_text(encoding="utf-8"))
    rows = []
    for r in results:
        run_name, rest = r["node"].split("/", 1)
        run = RUNS / run_name
        node = run / "work" / rest
        tag = re.sub(r"[^A-Za-z0-9]+", "_", r["node"])[-90:]
        d = args.out / tag
        a = arguments(run, node)
        img = nib.load(a["input"])
        data = np.asanyarray(img.dataobj).astype(np.float64)
        b_ants = np.asanyarray(nib.load(d / "ants_b.nii.gz").dataobj).astype(np.float64)
        b_exc = np.asanyarray(nib.load(d / "excluded_b.nii.gz").dataobj).astype(np.float64)
        brain = brain_mask(run, img, node)
        region = brain if brain is not None else data > 0
        ratio = b_exc[region] / b_ants[region]
        scale = float(np.median(ratio))
        shape = np.abs(ratio / scale - 1.0)
        rec = {
            "node": r["node"],
            "region": "brain" if brain is not None else "head",
            "scale": scale,
            "shape_max": float(shape.max()),
            "shape_p99": float(np.percentile(shape, 99)),
            "shape_median": float(np.median(shape)),
        }
        wm = wm_mask(run, img)
        if wm is not None and wm.sum() > 100:
            for label, b in (("ants", b_ants), ("excluded", b_exc)):
                c = data[wm] / b[wm]
                rec[f"wm_cv_{label}"] = float(c.std() / c.mean())
            rec["wm_cv_input"] = float(data[wm].std() / data[wm].mean())
        # fMRIPrep's final WM map is FAST's on the image ANTs' N4 corrected, so it favours
        # ANTs' field; the final N4's own weight (the brain extraction's WM posterior, computed
        # before it) is a region neither field shaped.
        if a["weight"] is not None and "inu_n4_final" in r["node"]:
            w = np.asanyarray(nib.load(a["weight"]).dataobj) > 0.9
            if w.sum() > 100:
                for label, b in (("ants", b_ants), ("excluded", b_exc)):
                    c = data[w] / b[w]
                    rec[f"weight_cv_{label}"] = float(c.std() / c.mean())
                rec["weight_cv_input"] = float(data[w].std() / data[w].mean())
        rows.append({**r, "analysis": rec})
        print(json.dumps(rec), flush=True)
    (args.out / "analysis.json").write_text(json.dumps(rows, indent=1), encoding="utf-8")
    print(
        "| Node | zero / negative voxels used (of included) | global scale | shape change: median, p99, max "
        "| ANTs' thread band: p99, max | WM CV: input, ANTs, excluded | weight > 0.9 CV: input, "
        "ANTs, excluded |"
    )
    print("|---|---|---|---|---|---|---|")
    for row in rows:
        x, s = row["analysis"], row["shrunk"]
        bb = row.get("band_brain") or row.get("band_head") or {}
        cv = (
            f"{x['wm_cv_input']:.4f}, {x['wm_cv_ants']:.4f}, {x['wm_cv_excluded']:.4f}"
            if "wm_cv_ants" in x
            else "–"
        )

        wcv = (
            f"{x['weight_cv_input']:.4f}, {x['weight_cv_ants']:.4f}, {x['weight_cv_excluded']:.4f}"
            if "weight_cv_ants" in x
            else "–"
        )

        def g(v):
            return "–" if v is None else f"{v:.2g}"

        print(
            f"| `{row['node']}` | {s['zero']} / {s['negative']} of {s['included']} | "
            f"{x['scale']:.4f} | "
            f"{g(x['shape_median'])}, {g(x['shape_p99'])}, {g(x['shape_max'])} | "
            f"{g(bb.get('p99'))}, {g(bb.get('max'))} | {cv} | {wcv} |"
        )
    return 0


if __name__ == "__main__":
    sys.exit(main())
