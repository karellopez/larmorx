# SPDX-License-Identifier: Apache-2.0
"""mcflirt's own spread on recorded runs of the `mri-hmc` record, and where larmorx lies in it.

mcflirt is deterministic, so its own spread is measured by perturbing its inputs in their last
bits and comparing its matrices with its own unperturbed (recorded) ones. For each recorded run:

- ``float32``: the series and the reference as float32 with the same values (a control: mcflirt
  reads every image as float, so its matrices must be the recorded ones as text);
- ``lastbit-<k>`` (``k`` = 1 .. ``--seeds``): the series and the reference as float32 with every
  nonzero voxel moved one unit in the last place up or down at random (seeded; zero stays zero,
  so no background voxel becomes a subnormal); about 1e-7 of each value;
- ``noise``, ``noise2``, ``noise3``, ``reverse``: the band of ``parity/mri_hmc_band.py`` (uniform
  noise of +-0.5 intensity units, three realisations; the volumes in reverse order), which the
  suite uses to widen the thresholds of other runs; reported beside the last-bit band.

Every mcflirt run goes through the oracle cache, never kept (it measures the original's own
spread); larmorx's clean-room original runs on the same inputs in ``reproduce`` mode, as the
record compares it. The measures are the record's: the matrices' RMS deviation (Jenkinson,
radius 80 mm, about the reference's field-of-view centre; its 95th percentile), the median
parameter differences, and for a ramp (``DEGENERATE`` in the suite) the identifiable
translation's largest difference and, where the test data's sidecar holds the applied motion,
the distance from it. Black box: only the programs' outputs are read.

    python -P hmc_spread.py <work> [--case <recorded run>]... [--seeds 8] [--out <summary.json>]
        (PYTHONPATH=<checkout>/python:<checkout>/validation/src, LARMORX_WORKSPACE set)
"""

from __future__ import annotations

import argparse
import json
import os
import shutil
import sys
from pathlib import Path

import numpy as np
from larmorx_validation import oracle_cache
from larmorx_validation.parity import defects, mri_hmc
from larmorx_validation.parity.mri_hmc import (
    DEGENERATE,
    TESTDATA,
    _fov_centre,
    _read_mats,
    _run_larmorx,
    decompose,
    rms_deviation,
)
from larmorx_validation.parity.mri_hmc_band import SEED, _perturb

REPO = Path(__file__).resolve().parents[3]
WORKSPACE = Path(os.environ.get("LARMORX_WORKSPACE", REPO.parent))
ROOT = WORKSPACE / "oracles" / "fsl-6.0.7.23" / "mcflirt-haswell"
INPUTS = WORKSPACE / "oracles" / "inputs" / "mri-hmc-spread"
CASES = ("syn/orientation/ras-unflipped/reffile", "syn/content/ramp/reffile")
BAND = ("noise", "noise2", "noise3", "reverse")
NEVER = "measures mcflirt's own spread: its inputs perturbed (hmc_spread.py)"


def _testdata(path: str) -> Path:
    import larmorx_testdata as td

    return Path(td.get(path.split(TESTDATA, 1)[1]))


def _as_float32(src: Path, dst: Path, rng: np.random.Generator | None) -> None:
    import nibabel as nib

    img = nib.load(src)
    data = np.asanyarray(img.dataobj).astype(np.float32)
    if rng is not None:
        up = rng.integers(0, 2, data.shape).astype(bool)
        moved = np.where(
            up,
            np.nextafter(data, np.float32(np.inf)),
            np.nextafter(data, np.float32(-np.inf)),
        )
        data = np.where(data != 0, moved, data).astype(np.float32)
    header = img.header.copy()
    header.set_data_dtype(np.float32)
    header.set_slope_inter(1.0, 0.0)
    nib.Nifti1Image(np.ascontiguousarray(data), img.affine, header).to_filename(dst)


def _inputs(case: str, kind: str, series: Path, ref: Path) -> tuple[Path, Path]:
    d = INPUTS / case / kind
    d.mkdir(parents=True, exist_ok=True)
    s, r = d / "series.nii.gz", d / "ref.nii.gz"
    if s.is_file() and r.is_file():
        return s, r
    if kind == "float32":
        _as_float32(series, s, None)
        _as_float32(ref, r, None)
    elif kind.startswith("lastbit-"):
        k = int(kind.removeprefix("lastbit-"))
        _as_float32(series, s, np.random.default_rng([SEED, k, 0]))
        _as_float32(ref, r, np.random.default_rng([SEED, k, 1]))
    else:
        made = _perturb(kind, series, ref, d)
        assert made is not None
        s, r = made
    return s, r


def _args(argv: list[str], series: Path, ref: Path, out: Path) -> list[str]:
    args = []
    for i, a in enumerate(argv):
        prev = argv[i - 1] if i else ""
        if prev == "-in":
            args.append(str(series))
        elif prev in ("-reffile", "-r"):
            args.append(str(ref))
        elif prev in ("-out", "-o"):
            args.append(str(out))
        else:
            args.append(a)
    return args


def _compare(a: list, b: list, centre: np.ndarray, axis: int | None) -> dict:
    devs = np.array([rms_deviation(a[i], b[i], centre) for i in range(len(a))])
    pa = np.array([decompose(m, centre) for m in a])
    pb = np.array([decompose(m, centre) for m in b])
    out = {
        "rms_p95": float(np.percentile(devs, 95)),
        "rms_median": float(np.median(devs)),
        "rms_max": float(devs.max()),
        "param_mm_median": float(np.median(np.abs(pa[:, 3:] - pb[:, 3:]))),
        "param_deg_median": float(np.median(np.degrees(np.abs(pa[:, :3] - pb[:, :3])))),
    }
    if axis is not None:
        out["identifiable_mm_max"] = float(np.abs(pa[:, axis] - pb[:, axis]).max())
    return out


def _truth_distance(mats: list, truth: list | None, centre: np.ndarray, axis: int | None) -> dict:
    if truth is None or len(truth) != len(mats):
        return {}
    t = [np.array(m) for m in truth]
    devs = [rms_deviation(t[i], mats[i], centre) for i in range(len(mats))]
    out = {"truth_rms_mean": float(np.mean(devs))}
    if axis is not None:
        pt = np.array([decompose(m, centre) for m in t])
        pm = np.array([decompose(m, centre) for m in mats])
        out["truth_identifiable_mm_max"] = float(np.abs(pt[:, axis] - pm[:, axis]).max())
    return out


def measure(case: str, work: Path, seeds: int) -> dict:
    run = json.loads((ROOT / "runs" / case / "run.json").read_text(encoding="utf-8"))
    argv = list(run["steps"][0]["argv"][1:])
    env = dict(run["env"])
    fsl = Path(env["FSLDIR"])
    series = _testdata(run["inputs"]["in"]["path"])
    ref = _testdata(run["inputs"]["ref"]["path"])
    centre = _fov_centre(ref)
    axis = DEGENERATE.get(case) if case in DEGENERATE else None
    sidecar = series.with_name(series.name.replace(".nii.gz", ".json"))
    truth = (
        json.loads(sidecar.read_text(encoding="utf-8")).get("expected_fsl_mat")
        if sidecar.is_file()
        else None
    )
    recorded, recorded_text = _read_mats(ROOT / "runs" / case / "out_mcf.nii.gz.mat")

    def larmorx(args: list[str], d: Path) -> tuple[list, list]:
        code, _, err = _run_larmorx(args, env, defects.REPRODUCE)
        if code != 0:
            raise SystemExit(f"{case}: larmorx exit {code}: {err[-300:]}")
        return _read_mats(d / "out_mcf.nii.gz.mat")

    # larmorx on the recorded inputs, as the record runs it.
    d0 = work / case / "unperturbed" / "larmorx"
    shutil.rmtree(d0, ignore_errors=True)
    d0.mkdir(parents=True)
    lx0, lx0_text = larmorx(_args(argv, series, ref, d0 / "out_mcf.nii.gz"), d0)
    result = {
        "case": case,
        "recorded": str(ROOT.relative_to(WORKSPACE) / "runs" / case),
        "larmorx_vs_mcflirt": _compare(recorded, lx0, centre, axis),
        "mcflirt_truth": _truth_distance(recorded, truth, centre, axis),
        "larmorx_truth": _truth_distance(lx0, truth, centre, axis),
        "larmorx_identical_text": sum(x == y for x, y in zip(recorded_text, lx0_text, strict=True)),
        "perturbations": [],
    }
    kinds = ["float32", *(f"lastbit-{k}" for k in range(1, seeds + 1)), *BAND]
    for kind in kinds:
        s, r = _inputs(case, kind, series, ref)
        dm = work / case / kind / "mcflirt"
        dl = work / case / kind / "larmorx"
        for d in (dm, dl):
            shutil.rmtree(d, ignore_errors=True)
            d.mkdir(parents=True)
        p = oracle_cache.run(
            [str(fsl / "bin" / "mcflirt"), *_args(argv, s, r, dm / "out_mcf.nii.gz")],
            env=env,
            cwd=dm,
            outputs=[dm],
            policy=oracle_cache.Policy(threads=1, family="fsl", never=NEVER),
            runner="hmc-spread",
            timeout=600,
        )
        if p.returncode != 0:
            raise SystemExit(f"{case} {kind}: mcflirt exit {p.returncode}")
        mc, mc_text = _read_mats(dm / "out_mcf.nii.gz.mat")
        lx, _ = larmorx(_args(argv, s, r, dl / "out_mcf.nii.gz"), dl)
        if kind == "reverse":
            mc, mc_text, lx = mc[::-1], mc_text[::-1], lx[::-1]
        entry = {
            "kind": kind,
            "family": "band" if kind in BAND else ("control" if kind == "float32" else "lastbit"),
            "mcflirt_vs_mcflirt": _compare(recorded, mc, centre, axis),
            "mcflirt_identical_text": sum(
                x == y for x, y in zip(recorded_text, mc_text, strict=True)
            ),
            "larmorx_vs_its_mcflirt": _compare(mc, lx, centre, axis),
            "larmorx_vs_larmorx": _compare(lx0, lx, centre, axis),
            "mcflirt_truth": _truth_distance(mc, truth, centre, axis),
            "larmorx_truth": _truth_distance(lx, truth, centre, axis),
        }
        result["perturbations"].append(entry)
        print(f"{case} {kind}: {json.dumps(entry)}", file=sys.stderr)
    return result


def summary(result: dict) -> dict:
    """The band (the worst of each family) against larmorx's distance from mcflirt."""
    out = {}
    for family in ("lastbit", "band"):
        runs = [p for p in result["perturbations"] if p["family"] == family]
        if not runs:
            continue
        keys = runs[0]["mcflirt_vs_mcflirt"].keys()
        out[family] = {
            "runs": len(runs),
            "mcflirt_own_worst": {k: max(p["mcflirt_vs_mcflirt"][k] for p in runs) for k in keys},
            "larmorx_vs_its_mcflirt_worst": {
                k: max(p["larmorx_vs_its_mcflirt"][k] for p in runs) for k in keys
            },
            "larmorx_own_worst": {k: max(p["larmorx_vs_larmorx"][k] for p in runs) for k in keys},
        }
        truths = [p["mcflirt_truth"] for p in runs if p["mcflirt_truth"]]
        if truths:
            out[family]["mcflirt_truth_range"] = {
                k: [min(t[k] for t in truths), max(t[k] for t in truths)] for k in truths[0]
            }
            lt = [p["larmorx_truth"] for p in runs if p["larmorx_truth"]]
            out[family]["larmorx_truth_range"] = {
                k: [min(t[k] for t in lt), max(t[k] for t in lt)] for k in lt[0]
            }
    return out


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("work", type=Path)
    ap.add_argument("--case", action="append")
    ap.add_argument("--seeds", type=int, default=8)
    ap.add_argument("--out", type=Path)
    a = ap.parse_args()
    os.environ.setdefault("LARMORX_MCFLIRT_ORACLE", str(ROOT))
    assert mri_hmc.oracle_dir() is not None
    results = []
    for case in a.case or CASES:
        r = measure(case, a.work, a.seeds)
        r["summary"] = summary(r)
        results.append(r)
        print(json.dumps({"case": case, **r["summary"]}, indent=1))
    if a.out:
        a.out.parent.mkdir(parents=True, exist_ok=True)
        a.out.write_text(json.dumps(results, indent=1) + "\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    sys.exit(main())
