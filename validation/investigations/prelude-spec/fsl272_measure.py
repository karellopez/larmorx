# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
"""FSL-272, rule D (specs/prelude.md §4.2, §9; spec writer, 2026-11-06): does leaving NaN out of
the phase extrema help? FSL takes the extrema from the first value on (§4.2), so a NaN first
phase value makes both NaN, and the range check (3π) and the rewrap (1.001π) are both skipped.

For each phase map (fMRIPrep 25.2.6's prelude calls on the oracle runs, each phase moved to
[0, 2π) as a scanner's phase difference in radians comes, and a synthetic one), with its call's
magnitude and mask, three FSL 6.0.7.23 runs through the oracle cache (black box):
  R  the phase as it is (the extrema finite: FSL rewraps it to [-π, π]);
  A  the same with the first value in FSL's order NaN (both x ends of the first row, background):
     FSL does not rewrap;
  B  the same NaN, the phase rewrapped beforehand (what FSL does with NaN left out of the extrema).
The unwrapped phases are compared inside R's mask, the NaN voxels left out: B against R (the
correction) and A against R (FSL), as the largest difference and the share of voxels that differ
by more than 1e-4 rad, and whether A's difference is a constant multiple of 2π per region. Then a
phase of range 4π with a NaN first value: FSL processes it (A), the correction stops with FSL's
own range error.

Usage (inside scripts/orchestration/boxed.sh 12, PYTHONPATH=python:validation/src, python -P):
  fsl272_measure.py <work dir>
"""

import shutil
import sys
from pathlib import Path

import nibabel as nib
import numpy as np

from larmorx_validation.parity import fsl_prelude_replica as P

TWO_PI = 2.0 * np.pi


def _load(p: Path) -> tuple[np.ndarray, nib.Nifti1Image]:
    img = nib.load(str(p))
    return np.asarray(img.dataobj, np.float32), img


def _save_like(img: nib.Nifti1Image, data: np.ndarray, p: Path) -> None:
    h = img.header.copy()
    h.set_data_dtype(np.float32)
    h.set_slope_inter(1.0, 0.0)
    nib.save(nib.Nifti1Image(data.astype(np.float32), img.affine, h), str(p))


def _nan_first(a: np.ndarray) -> np.ndarray:
    b = a.copy()
    b[0, 0, 0, ...] = np.nan
    b[-1, 0, 0, ...] = np.nan
    return b


def _rewrap(a: np.ndarray) -> np.ndarray:
    # §6.3's wrap, in double, stored in single precision; NaN stays NaN
    x = a.astype(np.float64)
    return (x - TWO_PI * np.round(x / TWO_PI)).astype(np.float32)


def _run(d: Path, args: list[str]) -> tuple[int, str]:
    p = P.oracle_run(args, cwd=d, settings={})
    return p.returncode, p.stderr


def one(work: Path, name: str, phase: np.ndarray, img, files: dict[str, Path], args: list[str]):
    out = {}
    for kind in ("R", "A", "B"):
        d = work / name / kind
        shutil.rmtree(d, ignore_errors=True)
        d.mkdir(parents=True)
        for fname, src in files.items():
            shutil.copy(src, d / fname)
        ph = phase if kind == "R" else _nan_first(phase if kind == "A" else _rewrap(phase))
        pname = next(a.split("=", 1)[1] for a in args if a.startswith(("--phase=", "-p=")))
        _save_like(img, ph, d / pname)
        a = [x for x in args if not x.startswith(("--unwrap=", "--out=", "-o=", "-u="))]
        code, err = _run(d, [*a, "--unwrap=uw", "--savemask=mk"])
        if code != 0:
            print(f"{name} {kind}: FSL exit {code}: {err.strip()[:200]}")
            return None
        out[kind] = (_load(next(d.glob("uw.nii*")))[0], _load(next(d.glob("mk.nii*")))[0])
    r, mask = out["R"]
    inside = (mask > 0.5) & np.isfinite(phase)
    inside[0, 0, 0] = inside[-1, 0, 0] = False
    res = {}
    for kind in ("A", "B"):
        u = out[kind][0]
        dlt = (u - r)[inside].astype(np.float64)
        big = np.abs(dlt) > 1e-4
        k = np.round(dlt / TWO_PI)
        res[kind] = (
            float(np.abs(dlt).max()) if dlt.size else 0.0,
            float(big.mean()) if dlt.size else 0.0,
            bool(np.all(np.abs(dlt - TWO_PI * k) < 1e-3)),
            int(np.unique(k).size),
        )
    print(
        f"{name}: {int(inside.sum())} voxels; correction B vs R: max {res['B'][0]:.3g} rad, "
        f"{100 * res['B'][1]:.2f} % > 1e-4; FSL A vs R: max {res['A'][0]:.3g} rad, "
        f"{100 * res['A'][1]:.2f} % > 1e-4, multiples of 2π {res['A'][2]} ({res['A'][3]} values)"
    )
    return res


def main() -> None:
    work = Path(sys.argv[1])
    work.mkdir(parents=True, exist_ok=True)
    rows = []
    for run, node in P.fmriprep_calls():
        args, files = P.fmriprep_args(run, node)
        pname = next(a.split("=", 1)[1] for a in args if a.startswith("--phase="))
        phase, img = _load(files[pname])
        phase = np.mod(phase.astype(np.float64), TWO_PI).astype(np.float32)
        tag = f"{run.split('-noreconall')[0]}-{node.parts[-3]}"
        rows.append(
            one(work, tag, phase, img, {k: v for k, v in files.items() if k != pname}, args)
        )
    # synthetic: the suite's smooth phase over the head phantom, wrapped into [0, 2π)
    shape = (40, 36, 24)
    true = P._true_phase(shape, 11)
    mag = P._magnitude(shape, 11).astype(np.float32)
    syn = work / "synthetic-src"
    syn.mkdir(exist_ok=True)
    P._save(syn / "mag.nii.gz", mag)
    P._save(syn / "ph.nii.gz", np.mod(true, TWO_PI).astype(np.float32))
    phase, img = _load(syn / "ph.nii.gz")
    rows.append(
        one(
            work,
            "synthetic",
            phase,
            img,
            {"mag.nii.gz": syn / "mag.nii.gz"},
            ["--phase=ph.nii.gz", "--abs=mag.nii.gz"],
        )
    )
    # the range check: a phase of range 4π
    big = work / "range"
    shutil.rmtree(big, ignore_errors=True)
    big.mkdir()
    P._save(big / "mag.nii.gz", mag)
    r4 = (np.mod(true, TWO_PI) * 2.0).astype(np.float32)
    for kind, ph in (("as-is", r4), ("nan-first", _nan_first(r4))):
        d = big / kind
        d.mkdir()
        shutil.copy(big / "mag.nii.gz", d / "mag.nii.gz")
        P._save(d / "ph.nii.gz", ph)
        code, err = _run(d, ["--phase=ph.nii.gz", "--abs=mag.nii.gz", "--unwrap=uw"])
        print(f"range 4π, {kind}: FSL exit {code}; {' '.join(err.split())[:120]}")
    done = [r for r in rows if r is not None]
    same_b = sum(1 for r in done if r["B"][0] == 0.0)
    diff_a = sum(1 for r in done if r["A"][1] > 0)
    print(
        f"maps: {len(done)}; the correction equals R in {same_b}; FSL's A differs from R in {diff_a}"
    )


main()
