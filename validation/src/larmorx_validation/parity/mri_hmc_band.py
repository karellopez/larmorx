# SPDX-License-Identifier: Apache-2.0
"""mcflirt's own variability band (``specs/mcflirt.md`` §14), for the ``mri-hmc`` parity suite.

mcflirt is deterministic: repeated runs are byte-identical, so its variability has to be
measured by perturbing the problem in ways that leave it essentially unchanged and comparing
mcflirt's matrices with its own unperturbed ones:

- ``noise``: the series as float32 with uniform noise of ±0.5 intensity units added (half an
  integer quantisation step);
- ``reverse``: the volumes in reverse order (runs with a separate reference only; the matrices
  are put back in order);
- ``crop``: the top slice removed from the series and, when it has the same grid, from the
  reference (FSL-mm coordinates do not change).

Usage (needs FSL; black-box runs of the binary)::

    python -m larmorx_validation.parity.mri_hmc_band --mcflirt ~/fsl/bin/mcflirt \\
        [--cases real/ds000005/fmriprep ...] [--out <oracle>/band]

Without ``--mcflirt`` the finished runs are only summarised again.

Each run is recorded like the oracle runs (``run.json``: argv, environment, exit status, wall
time, input SHA-256); ``band.json`` summarises the deviations per case and perturbation.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import subprocess
import sys
import time
from pathlib import Path
from typing import Any

import numpy as np

from larmorx_validation.parity.mri_hmc import (
    RECORDED,
    _fov_centre,
    _read_mats,
    _resolve,
    decompose,
    framewise_displacement,
    oracle_dir,
    rms_deviation,
)

PERTURBATIONS = ("noise", "reverse", "crop")
SEED = 20261010

#: The default cases: fMRIPrep's command on every real run, and synthetic runs with -mats.
DEFAULT_CASES = (
    "real/ds000005/fmriprep",
    "real/ds000117/fmriprep",
    "real/ds000122/fmriprep",
    "real/ds000210-rest-echo2/fmriprep",
    "real/ds000258-bigendian/fmriprep",
    "real/ds003345/fmriprep",
    "real/ds005040/fmriprep",
    "real/ds006010/fmriprep",
    "real/ds006736/fmriprep",
    "syn/motion/small/reffile",
    "syn/motion/small/default",
    "syn/motion/large/reffile",
    "syn/motion/identical-copies/reffile",
    "syn/motion/small/dof12",
    "syn/motion/small/dof7",
    "syn/motion/small/meanvol",
    "syn/geometry/tiny/default",
    "syn/content/background/default",
)


def _sha256(path: Path) -> str:
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for block in iter(lambda: f.read(1 << 20), b""):
            h.update(block)
    return h.hexdigest()


def _perturb(
    kind: str, series: Path, reference: Path | None, out: Path
) -> tuple[Path, Path | None] | None:
    import nibabel as nib

    img = nib.load(series)
    data = np.asanyarray(img.dataobj).astype(np.float32)
    if data.ndim == 3:
        data = data[..., None]
    ref_img = None if reference is None else nib.load(reference)
    new_ref = None
    if kind == "noise":
        rng = np.random.default_rng(SEED)
        data = data + rng.uniform(-0.5, 0.5, data.shape).astype(np.float32)
    elif kind == "reverse":
        if reference is None:
            return None
        data = data[..., ::-1]
    elif kind == "crop":
        if data.shape[2] < 4:
            return None
        data = data[:, :, :-1]
        if ref_img is not None and ref_img.shape[:3] == img.shape[:3]:
            rdata = np.asanyarray(ref_img.dataobj).astype(np.float32)
            rdata = rdata[:, :, :-1] if rdata.ndim == 3 else rdata[:, :, :-1, 0]
            new_ref = out / "ref.nii.gz"
            nib.Nifti1Image(rdata, ref_img.affine, ref_img.header).to_filename(new_ref)
    new = out / "series.nii.gz"
    header = img.header.copy()
    header.set_data_dtype(np.float32)
    header.set_slope_inter(1.0, 0.0)
    nib.Nifti1Image(np.ascontiguousarray(data), img.affine, header).to_filename(new)
    return new, (new_ref if new_ref is not None else reference)


def run_band(mcflirt: str, cases: list[str], out_root: Path) -> None:
    """Runs mcflirt on the perturbed inputs of every case (black-box runs)."""
    root = oracle_dir()
    if root is None:
        raise SystemExit("the recorded mcflirt runs were not found (LARMORX_MCFLIRT_ORACLE)")
    env = {
        "HOME": os.environ.get("HOME", ""),
        "PATH": "/usr/bin:/bin",
        "FSLDIR": str(Path(mcflirt).resolve().parents[1]),
        "FSLOUTPUTTYPE": "NIFTI_GZ",
    }
    for name in cases:
        run = json.loads((root / "runs" / name / "run.json").read_text(encoding="utf-8"))
        argv = run["steps"][0]["argv"][1:]
        series = Path(_resolve(run["inputs"]["in"]["path"]))
        reference = Path(_resolve(run["inputs"]["ref"]["path"])) if "ref" in run["inputs"] else None
        for kind in PERTURBATIONS:
            out = out_root / name / kind
            out.mkdir(parents=True, exist_ok=True)
            made = _perturb(kind, series, reference, out)
            if made is None:
                continue
            new_series, new_ref = made
            args = []
            for i, a in enumerate(argv):
                prev = argv[i - 1] if i else ""
                if prev in ("-in",):
                    args.append(str(new_series))
                elif prev in ("-reffile", "-r") and new_ref is not None:
                    args.append(str(new_ref))
                elif prev in ("-out", "-o"):
                    args.append(str(out / "out_mcf.nii.gz"))
                elif RECORDED in a:
                    args.append(_resolve(a))
                else:
                    args.append(a)
            if "-mats" not in args:
                args.append("-mats")
            start = time.perf_counter()
            p = subprocess.run([mcflirt, *args], env=env, capture_output=True, text=True)
            wall = time.perf_counter() - start
            (out / "stdout0.txt").write_text(p.stdout, encoding="utf-8")
            (out / "stderr0.txt").write_text(p.stderr, encoding="utf-8")
            record = {
                "name": f"band/{name}/{kind}",
                "notes": f"variability band: {kind} perturbation of {name}",
                "argv": [mcflirt, *args],
                "env": env,
                "exit": p.returncode,
                "wall_s": round(wall, 3),
                "seed": SEED,
                "inputs": {
                    "in": _sha256(new_series),
                    **({"ref": _sha256(new_ref)} if new_ref else {}),
                },
            }
            (out / "run.json").write_text(json.dumps(record, indent=1) + "\n", encoding="utf-8")
            print(f"{name} {kind}: exit {p.returncode}, {wall:.1f} s", file=sys.stderr)


def summarise(out_root: Path) -> dict[str, Any]:
    """Compares every finished band run with mcflirt's unperturbed run; writes ``band.json``."""
    root = oracle_dir()
    assert root is not None
    summary = []
    for record_path in sorted(out_root.glob("**/run.json")):
        record = json.loads(record_path.read_text(encoding="utf-8"))
        if record.get("exit") != 0:
            continue
        name, kind = record["name"].removeprefix("band/").rsplit("/", 1)
        run = json.loads((root / "runs" / name / "run.json").read_text(encoding="utf-8"))
        ref_arg = run["inputs"].get("ref", run["inputs"]["in"])["path"]
        centre = _fov_centre(Path(_resolve(ref_arg)))
        oracle_mats, _ = _read_mats(root / "runs" / name / "out_mcf.nii.gz.mat")
        mats, _ = _read_mats(record_path.parent / "out_mcf.nii.gz.mat")
        if kind == "reverse":
            mats = mats[::-1]
        idx = [
            i
            for i in range(min(len(mats), len(oracle_mats)))
            if np.isfinite(mats[i]).all() and np.isfinite(oracle_mats[i]).all()
        ]
        devs = np.array([rms_deviation(oracle_mats[i], mats[i], centre) for i in idx])
        pa = np.array([decompose(oracle_mats[i], centre) for i in idx])
        pb = np.array([decompose(mats[i], centre) for i in idx])
        fa, fb = framewise_displacement(pa), framewise_displacement(pb)
        fd_r = float(np.corrcoef(fa, fb)[0, 1]) if fa.std() > 0 and fb.std() > 0 else 1.0
        summary.append(
            {
                "case": name,
                "dataset": name.split("/")[1] if name.startswith("real/") else name,
                "perturbation": kind,
                "volumes": len(idx),
                "rms_median": float(np.median(devs)),
                "rms_p95": float(np.percentile(devs, 95)),
                "rms_max": float(devs.max()),
                "param_mm_median": float(np.median(np.abs(pa[:, 3:] - pb[:, 3:]))),
                "param_deg_median": float(np.median(np.degrees(np.abs(pa[:, :3] - pb[:, :3])))),
                "fd_r": fd_r,
                "fd_max_diff": float(np.abs(fa - fb).max()),
                "wall_s": record.get("wall_s"),
            }
        )
    band = {
        "generated_by": "larmorx_validation.parity.mri_hmc_band",
        "updated": time.strftime("%Y-%m-%dT%H:%M:%S"),
        "threshold_perturbations": list(THRESHOLD_PERTURBATIONS),
        "runs": summary,
    }
    (out_root / "band.json").write_text(json.dumps(band, indent=1) + "\n", encoding="utf-8")
    return band


#: The perturbations that set the parity thresholds: those that leave the geometry unchanged.
#: Cropping a slice changes the problem more (thin images can fail outright) and is reported.
THRESHOLD_PERTURBATIONS = ("noise", "reverse")


def band_for(case: str) -> dict[str, float] | None:
    """mcflirt's worst deviations from itself for ``case`` under the threshold perturbations
    (the lowest FD correlation for ``fd_r``)."""
    root = oracle_dir()
    path = None if root is None else root / "band" / "band.json"
    if path is None or not path.is_file():
        return None
    runs = [
        b
        for b in json.loads(path.read_text(encoding="utf-8"))["runs"]
        if b["case"] == case and b["perturbation"] in THRESHOLD_PERTURBATIONS
    ]
    if not runs:
        return None
    worst = {
        k: max(b[k] for b in runs)
        for k in ("rms_median", "rms_p95", "rms_max", "param_mm_median", "param_deg_median")
    }
    worst["fd_r"] = min(b.get("fd_r", 1.0) for b in runs)
    return worst


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(prog="python -m larmorx_validation.parity.mri_hmc_band")
    parser.add_argument("--mcflirt", help="the mcflirt binary (omit to only summarise)")
    parser.add_argument("--cases", nargs="+", default=list(DEFAULT_CASES))
    parser.add_argument("--out", help="output directory (default: <oracle>/band)")
    args = parser.parse_args(argv)
    root = oracle_dir()
    out = Path(args.out) if args.out else (root / "band" if root else Path("band"))
    if args.mcflirt:
        run_band(args.mcflirt, args.cases, out)
    summarise(out)
    return 0


if __name__ == "__main__":
    sys.exit(main())
