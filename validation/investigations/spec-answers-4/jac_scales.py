# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
"""specs/topup.md §7.3, FSL-273: applytopup ``--method=jac``'s Jacobian scales when the field's
voxel sizes differ from the images', shown with FSL's own runs only (black box; FSL 6.0.7.23
through the oracle cache in use mode).

FSL lays the field at the images' voxel indices and displaces by ``h t`` voxels whatever the
voxel sizes, so the resampled image ``S`` is the same in every run below; only the Jacobian
``J`` can differ. If FSL's Jacobian is ``1 + c g`` (``g = t py ∂h/∂y`` per voxel index), each
output's ratio to the reference run (``c = 1``) is ``r = (1 + c g) / (1 + g)``, and the ``g``
solved from two runs of different ``c``, ``g = (1 − r) / (r − c)``, agree.

- topup's coefficients ``tu`` (made on 6 mm voxels along y) on the same image data (the topup
  replica suite's ``half_up``) stored with 6, 4 and 3 mm voxels along y: FSL's ``c = dy / fy``
  (6/6, 4/6, 3/6);
- topup's field image ``tu_field`` (``--fout``) with its header's y voxel size 6, 4 or 3 mm, on
  the 6 mm image: FSL's ``c = dx / fy`` (6/6, 6/4, 6/3).

A Jacobian of the mapping applied has ``c = 1`` in every run.

Usage (inside scripts/orchestration/boxed.sh 8, PYTHONPATH=python:validation/src, python -P):
  jac_scales.py run <dir>                FSL's six runs
  jac_scales.py check <dir> [--larmorx <program> ...]
       the ratios and the solved g; with --larmorx, the default run of a larmorx program (e.g.
       ``larmorx fsl applytopup`` or ``larmorx-nc fsl applytopup``) in each run directory,
       compared with FSL's reference run (a Jacobian of the mapping applied gives it to
       single-precision rounding)
"""

import subprocess
import sys
from pathlib import Path

import nibabel as nib
import numpy as np

from larmorx_validation.parity import fsl_topup_replica as T

#: run name: (the image's y voxel size, the --topup argument, the field image's y voxel size, FSL's c)
RUNS = {
    "coef_y6": (6.0, "--topup=tu", None, 1.0),
    "coef_y4": (4.0, "--topup=tu", None, 4 / 6),
    "coef_y3": (3.0, "--topup=tu", None, 3 / 6),
    "image_f6": (6.0, "--topup=tu_field", 6.0, 1.0),
    "image_f4": (6.0, "--topup=tu_field", 4.0, 6 / 4),
    "image_f3": (6.0, "--topup=tu_field", 3.0, 6 / 3),
}
GROUPS = {"coef": ("coef_y6", "coef_y4", "coef_y3"), "image": ("image_f6", "image_f4", "image_f3")}


def _with_y(src: Path, dst: Path, dy: float) -> None:
    img = nib.load(str(src))
    data = np.asarray(img.dataobj, np.float32)
    aff = img.affine.copy()
    aff[:3, 1] = aff[:3, 1] * (dy / float(np.linalg.norm(aff[:3, 1])))
    out = nib.Nifti1Image(data, aff, img.header)
    out.set_qform(aff, 1)
    out.set_sform(aff, 1)
    nib.save(out, str(dst))


def _args(topup: str) -> list[str]:
    # trilinear: FSL resamples a single image's first volume trilinearly whatever --interp says
    # (FSL-132), larmorx's default with the method asked for; trilinear asks for FSL's own
    return [
        "--imain=img.nii.gz",
        "--inindex=1",
        "--datain=acq.txt",
        topup,
        "--out=hifi",
        "--method=jac",
        "--interp=trilinear",
    ]


def run(d: Path) -> None:
    T.applytopup_inputs()
    src = T.topup_outputs("y")
    for name, (dy, topup, fy, _) in RUNS.items():
        w = d / name
        w.mkdir(parents=True, exist_ok=True)
        _with_y(T.inputs() / "half_up.nii.gz", w / "img.nii.gz", dy)
        (w / "acq.txt").write_text(T.TEXTS["acq.txt"])
        for f in sorted(src.iterdir()):
            if f.name.startswith("tu_") and f.name != "tu_field.nii.gz":
                (w / f.name).write_bytes(f.read_bytes())
        if fy is None:
            (w / "tu_field.nii.gz").write_bytes((src / "tu_field.nii.gz").read_bytes())
        else:
            _with_y(src / "tu_field.nii.gz", w / "tu_field.nii.gz", fy)
        p = T.oracle_run(_args(topup), cwd=w, settings={}, program="applytopup")
        (w / "EXIT.txt").write_text(f"{p.returncode}\n")
        print(name, "exit", p.returncode)


def _load(p: Path) -> np.ndarray:
    return np.asarray(nib.load(str(p)).dataobj, np.float64)


def check(d: Path, larmorx: list[str]) -> None:
    for group, names in GROUPS.items():
        ref = _load(d / names[0] / "hifi.nii.gz")
        m = np.abs(ref) > 0.05 * np.abs(ref).max()
        print(f"{group}:")
        gs = []
        for name in names[1:]:
            c = RUNS[name][3]
            o = _load(d / name / "hifi.nii.gz")
            r = o[m] / ref[m]
            gs.append((1 - r) / (r - c))
            print(
                f"  FSL {name} / FSL {names[0]} (c = {c:.4f}): ratio p1 {np.percentile(r, 1):.6f} "
                f"p50 {np.median(r):.6f} p99 {np.percentile(r, 99):.6f}"
            )
        big = np.abs(gs[0]) > 0.02
        dg = np.abs(gs[0][big] - gs[1][big]) / np.abs(gs[0][big])
        print(
            f"  g solved from the two runs (|g| > 0.02, {big.sum()} voxels): relative difference "
            f"p50 {np.median(dg):.2e} p99 {np.percentile(dg, 99):.2e}; |g| p99 "
            f"{np.percentile(np.abs(gs[0]), 99):.4f}"
        )
        if larmorx:
            for name in names:
                w = d / name
                out = w / "lx_default"
                out.mkdir(exist_ok=True)
                args = [a.replace("--out=hifi", f"--out={out}/hifi") for a in _args(RUNS[name][1])]
                p = subprocess.run([*larmorx, *args], cwd=w, capture_output=True, text=True)
                o = _load(out / "hifi.nii.gz")
                rel = np.abs(o[m] - ref[m]) / np.abs(ref[m])
                named = sorted(
                    {ln.split()[3] for ln in p.stderr.splitlines() if "upstream defect" in ln}
                )
                print(
                    f"  larmorx's default {name} vs FSL {names[0]}: exit {p.returncode}, relative "
                    f"difference max {rel.max():.2e} p99 {np.percentile(rel, 99):.2e}; named {named}"
                )


def main() -> None:
    kind, d = sys.argv[1], Path(sys.argv[2])
    if kind == "run":
        run(d)
    else:
        rest = sys.argv[3:]
        check(d, rest[rest.index("--larmorx") + 1 :] if "--larmorx" in rest else [])


main()
