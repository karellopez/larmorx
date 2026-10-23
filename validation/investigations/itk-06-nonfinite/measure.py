# SPDX-License-Identifier: Apache-2.0
"""ITK-06: ITK's NIfTI reader (nifti_clib) reads NaN and ±infinity as 0. Which larmorx tools
and which real inputs it affects, and what keeping the non-finite values would do downstream.

1. **Real inputs.** Every image argument of every ANTs program node of the fMRIPrep 25.2.6
   oracle runs (``oracles/fmriprep-25.2.6/*/work/**/command.txt``; ``/work``, ``/out``,
   ``/data`` and ``/templateflow`` mapped to the run's directories), and every NIfTI file of
   the ``larmorx-testdata`` catalog's cache: the float images among them, and how many NaN,
   +inf and -inf values each holds (as stored, before scaling).
2. **Downstream.** fMRIPrep's raw T1w of ds000005, resampled to 2 mm by larmorx, with
   non-finite values put where they arise in practice (a NaN slab where a resampler marks
   "outside the field of view", a NaN voxel and an infinite voxel in the head), run in memory
   through larmorx's ANTs ports twice: as nifti_clib gives it (the values 0) and with the
   non-finite values kept (larmorx's in-memory Python API does not zero them). For each tool,
   how many output values are non-finite and how the finite ones differ.

Usage (from the larmorx checkout, larmorx built into ``python/``)::

    PYTHONPATH=python:validation/src python -P validation/investigations/itk-06-nonfinite/measure.py \\
        --out <scratch dir> [--threads 6]

It writes ``<out>/results.json`` and prints the tables.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import shlex
import sys
import warnings
from pathlib import Path

import nibabel as nib
import numpy as np

ANTS_PROGRAMS = {
    "antsRegistration",
    "antsApplyTransforms",
    "N4BiasFieldCorrection",
    "DenoiseImage",
    "ImageMath",
    "ThresholdImage",
    "Atropos",
    "antsAI",
    "antsAffineInitializer",
    "ResampleImage",
    "ResampleImageBySpacing",
    "SmoothImage",
    "MultiplyImages",
    "antsJointFusion",
    "CreateJacobianDeterminantImage",
}


def _workspace() -> Path:
    if env := os.environ.get("LARMORX_WORKSPACE"):
        return Path(env)
    for parent in Path(__file__).resolve().parents:
        if (parent / "oracles" / "fmriprep-25.2.6").is_dir():
            return parent
    raise FileNotFoundError("no workspace with oracles/fmriprep-25.2.6 (LARMORX_WORKSPACE)")


WS = _workspace()
RUNS = WS / "oracles" / "fmriprep-25.2.6"


def nonfinite(path: Path) -> dict | None:
    """NaN, +inf and -inf of a float NIfTI file as stored; None for other data types."""
    img = nib.load(path)
    if img.get_data_dtype().kind != "f":
        return None
    a = np.asanyarray(img.dataobj.get_unscaled())
    return {
        "dtype": str(a.dtype),
        "voxels": int(a.size),
        "nan": int(np.isnan(a).sum()),
        "posinf": int(np.isposinf(a).sum()),
        "neginf": int(np.isneginf(a).sum()),
    }


def run_inputs() -> list[dict]:
    """Every NIfTI argument of the ANTs nodes of the oracle runs."""
    seen: dict[Path, dict] = {}
    for run in sorted(RUNS.iterdir()):
        if not (run / "work").is_dir():
            continue
        info = (run / "RUN_INFO.txt").read_text(encoding="utf-8")
        bids = re.search(r"BIDS input: (.*)", info)
        roots = {
            "/work/": run / "work",
            "/out/": run / "out",
            "/templateflow/": WS / "oracles" / "templateflow",
            "/data/": Path(bids.group(1).strip()) if bids else run / "data",
        }
        for cmd in (run / "work").rglob("command.txt"):
            text = cmd.read_text(encoding="utf-8", errors="replace").strip()
            if not text:
                continue
            program = text.split()[0]
            if program not in ANTS_PROGRAMS:
                continue
            for token in re.split(r"[\s,\[\]]+", text):
                if not re.search(r"\.nii(\.gz)?$", token):
                    continue
                path = None
                for root, local in roots.items():
                    if token.startswith(root):
                        path = local / token[len(root) :]
                if path is None:
                    path = cmd.parent / token
                if not path.is_file():
                    continue
                shown = str(path.relative_to(WS)) if path.is_relative_to(WS) else str(path)
                rec = seen.setdefault(path, {"file": shown, "programs": set(), "runs": set()})
                rec["programs"].add(program)
                rec["runs"].add(run.name)
    out = []
    for path, rec in seen.items():
        nf = nonfinite(path)
        rec.update(programs=sorted(rec["programs"]), runs=sorted(rec["runs"]), stats=nf)
        out.append(rec)
    return sorted(out, key=lambda r: r["file"])


def catalog_files() -> list[dict]:
    import larmorx_testdata as td

    root = Path(td.cache_dir()) / "data"
    out = []
    for p in sorted(root.rglob("*.nii*")):
        nf = nonfinite(p)
        out.append({"file": str(p.relative_to(root)), "stats": nf})
    return out


def downstream(threads: int) -> list[dict]:
    """The tools on the 2 mm T1w with and without its non-finite values."""
    import larmorx as lx
    import larmorx_testdata as td

    raw = lx.ants.resample_image_by_spacing(
        str(td.get("openneuro/ds000005/sub-01/anat/sub-01_T1w.nii.gz")),
        [2.0, 2.0, 2.0],
        n_threads=threads,
    )
    finite = np.asarray(raw.data, np.float32).copy()
    kept = finite.copy()
    kept[:, :, :3] = np.nan  # a slab "outside the field of view"
    centre = tuple(n // 2 for n in kept.shape)
    kept[centre] = np.nan
    kept[centre[0] + 5, centre[1], centre[2]] = np.inf
    zeroed = np.where(np.isfinite(kept), kept, 0.0).astype(np.float32)
    head = zeroed > np.percentile(zeroed[zeroed > 0], 10)
    mask = lx.Image(head.astype(np.uint8), raw.affine)

    def as_image(a: np.ndarray) -> lx.Image:
        return lx.Image(np.asfortranarray(a), raw.affine)

    tools = {
        "SmoothImage (sigma 2 voxels)": lambda im: lx.ants.smooth_image(im, 2.0, n_threads=threads),
        "ResampleImageBySpacing (3 mm)": lambda im: lx.ants.resample_image_by_spacing(
            im, [3.0, 3.0, 3.0], n_threads=threads
        ),
        "antsApplyTransforms (identity, linear, 3 mm grid)": lambda im: lx.ants.apply_transforms(
            im,
            lx.ants.resample_image_by_spacing(as_image(finite), [3.0, 3.0, 3.0], smooth=False),
            [],
            n_threads=threads,
        ),
        "TruncateImageIntensity (0.025, 0.975)": lambda im: lx.ants.truncate_image_intensity(
            im, 0.025, 0.975, n_threads=threads
        ),
        "N4BiasFieldCorrection (shrink 2, 2 levels)": lambda im: lx.ants.n4_bias_field_correction(
            im, shrink_factor=2, convergence=((20, 20), 0.0), n_threads=threads
        ).corrected,
        "DenoiseImage (Rician)": lambda im: lx.ants.denoise_image(
            im, n_threads=threads
        ).denoised,
        "Atropos (K-means, 3 classes, head mask)": lambda im: lx.ants.atropos(
            im, mask, classes=3, n_threads=threads
        ).labels,
    }
    out = []
    for name, f in tools.items():
        rec: dict = {"tool": name}
        results = {}
        for label, data in (("zeroed (nifti_clib)", zeroed), ("kept", kept)):
            try:
                with warnings.catch_warnings():
                    warnings.simplefilter("ignore")
                    results[label] = np.asarray(f(as_image(data)).data, np.float64)
            except Exception as e:  # noqa: BLE001: what a tool does with them is the finding
                results[label] = f"{type(e).__name__}: {str(e)[:200]}"
        for label, r in results.items():
            if isinstance(r, str):
                rec[label] = {"error": r}
            else:
                rec[label] = {"values": int(r.size), "nonfinite": int((~np.isfinite(r)).sum())}
        a, b = results["zeroed (nifti_clib)"], results["kept"]
        if not isinstance(a, str) and not isinstance(b, str) and a.shape == b.shape:
            both = np.isfinite(a) & np.isfinite(b)
            d = np.abs(a - b)[both]
            scale = float(np.abs(a[np.isfinite(a)]).max() or 1.0)
            rec["finite_values_differing"] = int((d > 0).sum())
            rec["max_rel_diff_finite"] = float(d.max() / scale) if d.size else 0.0
        out.append(rec)
    return out


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--out", type=Path, required=True)
    ap.add_argument("--threads", type=int, default=6)
    args = ap.parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    res = {
        "run_inputs": run_inputs(),
        "catalog": catalog_files(),
        "downstream": downstream(args.threads),
    }
    (args.out / "results.json").write_text(json.dumps(res, indent=1), encoding="utf-8")
    floats = [r for r in res["run_inputs"] if r["stats"]]
    bad = [r for r in floats if r["stats"]["nan"] + r["stats"]["posinf"] + r["stats"]["neginf"]]
    print(
        f"oracle runs: {len(res['run_inputs'])} images read by ANTs programs, {len(floats)} "
        f"float, {len(bad)} with non-finite values"
    )
    for r in bad:
        print(f"  {r['file']}: {r['stats']} ({', '.join(r['programs'])})")
    cfl = [r for r in res["catalog"] if r["stats"]]
    cbad = [r for r in cfl if r["stats"]["nan"] + r["stats"]["posinf"] + r["stats"]["neginf"]]
    print(
        f"catalog: {len(res['catalog'])} NIfTI files, {len(cfl)} float, {len(cbad)} with "
        "non-finite values"
    )
    for r in cbad:
        print(f"  {r['file']}: {r['stats']}")
    print("| Tool | read as nifti_clib does (0) | kept | finite values that differ |")
    print("|---|---|---|---|")
    for r in res["downstream"]:
        def cell(x):
            return x.get("error") or f"{x['nonfinite']} of {x['values']} non-finite"
        print(
            f"| {r['tool']} | {cell(r['zeroed (nifti_clib)'])} | {cell(r['kept'])} | "
            f"{r.get('finite_values_differing', '–')} (max rel {r.get('max_rel_diff_finite', 0):.2g}) |"
        )
    return 0


if __name__ == "__main__":
    sys.exit(main())
