# SPDX-License-Identifier: Apache-2.0
"""Check ``specs/3dvolreg.md`` with its model (``model.py``) against AFNI's 3dvolreg binary.

Usage (inside a memory box; the AFNI binary is run as a black box, with a clean environment):

    scripts/orchestration/boxed.sh 8 python -P validation/investigations/3dvolreg-spec/check.py \
        <work folder> [case ...]

Each case writes an input (a synthetic moving series made here with seeded numbers, or a file
named in ``CASES``), runs AFNI's 3dvolreg and the model with the same options, and prints one
line: how many ``-1Dfile`` lines are identical, the largest parameter difference, the
``-1Dmatrix_save`` lines identical, the ``-maxdisp1D`` values identical, and the registered
volumes' largest difference relative to AFNI's largest value. ``AFNI_VOLREG`` names AFNI's
program (default ``<workspace>/oracles/afni-26.3.05/bin/3dvolreg``).
"""

from __future__ import annotations

import os
import subprocess
import sys
import time
from pathlib import Path

import nibabel as nib
import numpy as np
from scipy import ndimage

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import model as M  # noqa: E402

WORKSPACE = Path(os.environ.get("LARMORX_WORKSPACE", HERE.parents[3]))
AFNI = Path(os.environ.get("AFNI_VOLREG", WORKSPACE / "oracles/afni-26.3.05/bin/3dvolreg"))
SLICED = WORKSPACE / "oracles/inputs/afni-volreg/fmriprep"


def phantom(shape=(40, 44, 24), seed=7) -> np.ndarray:
    """A smooth head-like phantom (x, y, z order): ellipsoids and blobs, plain arithmetic."""
    rng = np.random.default_rng(seed)
    nx, ny, nz = shape
    x, y, z = np.meshgrid(np.arange(nx), np.arange(ny), np.arange(nz), indexing="ij")
    cx, cy, cz = (nx - 1) / 2 + 0.7, (ny - 1) / 2 - 0.4, (nz - 1) / 2 + 0.3
    r = ((x - cx) / (0.36 * nx)) ** 2 + ((y - cy) / (0.4 * ny)) ** 2 + ((z - cz) / (0.38 * nz)) ** 2
    v = np.where(r < 1, 600.0 + 300.0 * (1 - r), 0.0)
    for _ in range(14):
        px, py, pz = rng.uniform(0.3, 0.7, 3) * (nx, ny, nz)
        s = rng.uniform(1.5, 3.5)
        v += rng.uniform(-200, 400) * np.exp(-((x - px) ** 2 + (y - py) ** 2 + (z - pz) ** 2) / (2 * s * s)) * (r < 1)
    return ndimage.gaussian_filter(v, 0.8)


def moving_series(path: Path, *, n=5, deg=0.6, shift=(0.3, -0.2, 0.15), affine=None, dtype=np.float32,
                  shape=(40, 44, 24), voxel=(3.0, 3.0, 3.5)) -> None:
    base = phantom(shape)
    vols = []
    c = (np.array(shape) - 1) / 2
    for k in range(n):
        a = np.deg2rad(deg * k)
        R = np.array([[np.cos(a), -np.sin(a), 0], [np.sin(a), np.cos(a), 0], [0, 0, 1.0]])
        R = R @ np.array([[1, 0, 0], [0, np.cos(a / 3), -np.sin(a / 3)], [0, np.sin(a / 3), np.cos(a / 3)]])
        off = c - R @ c - np.array(shift) * k
        vols.append(ndimage.affine_transform(base, R, offset=off, order=3))
    data = np.stack(vols, axis=3)
    rng = np.random.default_rng(3)
    data += rng.normal(0, 4.0, data.shape)
    if affine is None:
        affine = np.diag([-voxel[0], -voxel[1], voxel[2], 1.0])
        affine[:3, 3] = [60.0, 70.0, -40.0]
    data = np.round(data).astype(dtype) if np.issubdtype(dtype, np.integer) else data.astype(dtype)
    img = nib.Nifti1Image(data, affine)
    img.header.set_xyzt_units("mm", "sec")
    img.header["pixdim"][4] = 2.0
    img.set_sform(affine, 1)
    img.set_qform(affine, 1)
    img.to_filename(path)


FMRIPREP = ("-Fourier", "-twopass", "-zpad", "4")

#: name -> (input maker or file, AFNI options, model settings)
CASES = {
    "heptic": ("syn", (), {}),
    "linear": ("syn", ("-linear",), {"method": "linear"}),
    "cubic": ("syn", ("-cubic",), {"method": "cubic"}),
    "quintic": ("syn", ("-quintic",), {"method": "quintic"}),
    "fourier": ("syn", ("-Fourier",), {"method": "fourier"}),
    "final-nn": ("syn", ("-final", "NN"), {"final": "nn"}),
    "noclip-zpad0": ("syn", ("-noclip", "-zpad", "0"), {"clip": False, "zpad": 0}),
    "base-2": ("syn", ("-base", "2"), {"base_index": 2}),
    "twopass-heptic": ("syn", ("-twopass",), {"twopass": True}),
    "fmriprep": ("syn", FMRIPREP, {"method": "fourier", "twopass": True}),
    "ras-int16": ("syn-ras-int16", (), {}),
    "ras-int16-fmriprep": ("syn-ras-int16", FMRIPREP, {"method": "fourier", "twopass": True}),
    # AFNI reads -nocoarserot as -noclip (AFNI-86): the clipping off, the rotation search on
    "nocoarserot-cubic": ("syn", ("-cubic", "-twopass", "-nocoarserot", "-nomaxdisp"),
                          {"method": "cubic", "twopass": True, "clip": False, "maxdisp": False}),
    "shift-only-cubic": ("syn", ("-cubic", "-twopass", "-coarse", "2", "1", "-nomaxdisp"),
                         {"method": "cubic", "twopass": True, "coarse_del": 2, "coarse_num": 1, "maxdisp": False}),
    "ds000005-fmriprep": ("ds000005-run-01_sliced.nii.gz", FMRIPREP, {"method": "fourier", "twopass": True}),
    "ds000005-heptic": ("ds000005-run-01_sliced.nii.gz", (), {}),
}


def make_input(kind: str, d: Path) -> Path:
    if kind == "syn":
        p = d / "syn.nii.gz"
        if not p.exists():
            moving_series(p)
        return p
    if kind == "syn-ras-int16":
        p = d / "syn_ras_int16.nii.gz"
        if not p.exists():
            aff = np.diag([3.0, 3.0, 3.5, 1.0])
            aff[:3, 3] = [-60.0, -70.0, -40.0]
            moving_series(p, affine=aff, dtype=np.int16)
        return p
    return SLICED / kind


def run_afni(inp: Path, opts, d: Path) -> float:
    env = {"HOME": str(d), "PATH": "/usr/bin:/bin", "AFNI_DONT_LOGFILE": "YES", "AFNI_NIFTI_TYPE_WARN": "NO"}
    for f in ("a.nii", "a.1D", "a.aff12.1D", "a_md.1D", "a_md_delt.1D"):
        (d / f).unlink(missing_ok=True)
    t = time.time()
    p = subprocess.run([str(AFNI), *opts, "-prefix", "a.nii", "-1Dfile", "a.1D", "-1Dmatrix_save", "a",
                        "-maxdisp1D", "a_md.1D", str(inp)], cwd=d, env=env, capture_output=True, text=True)
    if p.returncode != 0:
        raise RuntimeError(p.stderr[-500:])
    return time.time() - t


def lines(path: Path) -> list[str]:
    return [x for x in path.read_text(encoding="utf-8").splitlines() if not x.startswith("#")]


def check(name: str, work: Path) -> str:
    kind, opts, kw = CASES[name]
    d = work / name
    d.mkdir(parents=True, exist_ok=True)
    inp = make_input(kind, work)
    if not inp.exists():
        return f"{name}: input {inp} missing"
    ta = run_afni(inp, opts, d)
    t = time.time()
    res = M.volreg(str(inp), M.Settings(**kw))
    tm = time.time() - t
    M.write_1d(res, d / "m.1D")
    M.write_matrices(res, d / "m.aff12.1D")
    a1, m1 = lines(d / "a.1D"), lines(d / "m.1D")
    same1 = sum(x == y for x, y in zip(a1, m1))
    pa = np.array([[float(v) for v in x.split()] for x in a1])
    pm = np.array([[float(v) for v in x.split()] for x in m1])
    dpar = float(np.max(np.abs(pa - pm)))
    am, mm = lines(d / "a.aff12.1D"), lines(d / "m.aff12.1D")
    samem = sum(x == y for x, y in zip(am, mm))
    md = ""
    if (d / "a_md.1D").exists() and res.maxdisp:
        amd = [float(x) for x in lines(d / "a_md.1D")]
        same_md = sum(abs(x - y) < 5e-4 for x, y in zip(amd, res.maxdisp))
        ade = [float(x) for x in lines(d / "a_md_delt.1D")]
        same_de = sum(abs(x - y) < 5e-4 for x, y in zip(ade, res.maxdelta))
        md = f" maxdisp {same_md}/{len(amd)} delta {same_de}/{len(ade)}"
    ad = np.asarray(nib.load(d / "a.nii").dataobj).astype(np.float64)
    mdat = np.stack([v.transpose(2, 1, 0) for v in res.registered], axis=3).astype(np.float64)
    if ad.ndim == 3:
        ad = ad[..., None]
    rel = float(np.max(np.abs(ad - mdat)) / max(np.max(np.abs(ad)), 1e-30))
    bits = int(np.sum(ad == mdat))
    return (f"{name}: 1Dfile {same1}/{len(a1)} identical (max |diff| {dpar:.4g}); matrices {samem}/{len(am)};"
            f"{md}; volumes max rel diff {rel:.3g}, {bits}/{ad.size} voxels identical; AFNI {ta:.1f} s, model {tm:.1f} s")


def main() -> int:
    work = Path(sys.argv[1])
    names = sys.argv[2:] or list(CASES)
    for n in names:
        try:
            print(check(n, work), flush=True)
        except Exception as e:  # noqa: BLE001
            print(f"{n}: ERROR {type(e).__name__}: {e}", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
