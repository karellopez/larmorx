# SPDX-License-Identifier: Apache-2.0
"""AFNI-75's guard (``specs/3dUnifize.md`` §9.1, §10) through the clean-room original and the
replica, side by side: ``-EPI`` on 4D (and one-volume) series of tiny voxels.

Black-box only: ``larmorx afni 3dUnifize`` (the original, ``LARMORX_IMPLEMENTATION=original``)
and the replica (``LARMORX_IMPLEMENTATION=replica``) are run as programs and their messages
compared; AFNI runs only where the guard does not refuse (it would fill the machine).

    python -P unifize_guard.py <work> [--larmorx target/release/larmorx] [--afni <bin dir>]

Prints, per case and mode, each program's exit status, the AFNI-75 lines and whether the outputs
are identical (where both run).
"""

from __future__ import annotations

import argparse
import hashlib
import os
import subprocess
from pathlib import Path

import nibabel as nib
import numpy as np

#: (case, voxel size in the header's units, the units' xyzt code, volumes)
CASES = (
    ("um3", 3.0, 3, 3),  # 3 µm voxels: radius 6100
    ("um1", 1.0, 3, 3),  # 1 µm: radius 18300
    ("mm0.05", 0.05, 2, 3),  # 0.05 mm: radius 366, no hint
    ("um3-one", 3.0, 3, 1),  # one volume: refused as well
    ("mm0.3", 0.3, 2, 3),  # radius 61: runs
    ("mm3", 3.0, 2, 3),  # radius 6.1: runs
)
SHAPE = (24, 24, 16)


def series(size: float, units: int, nt: int, path: Path) -> Path:
    """A head-like EPI phantom (plain arithmetic, seeded noise) with voxels of ``size`` in the
    header's ``units`` (xyzt code: 2 mm, 3 µm; seconds added)."""
    x, y, z = np.meshgrid(*(np.arange(n) for n in SHAPE), indexing="ij")
    r2 = ((x - 11.5) / 9) ** 2 + ((y - 11.5) / 10) ** 2 + ((z - 7.5) / 6) ** 2
    base = 800.0 * np.clip(1.2 - r2, 0, 1) + 300.0 * (r2 < 0.5)
    rng = np.random.default_rng(75)
    data = np.stack([base * (1 + 0.01 * t) + rng.normal(0, 5, SHAPE) for t in range(nt)], -1)
    affine = np.diag([size, size, size, 1.0])
    affine[:3, 3] = [-(n - 1) * size / 2 for n in SHAPE]
    img = nib.Nifti1Image(data.astype(np.float32), affine)
    img.header["xyzt_units"] = units | 8
    img.header["pixdim"][4] = 2.0
    nib.save(img, path)
    return path


def digest(p: Path) -> str:
    if not p.exists():
        return "-"
    return hashlib.sha256(np.asarray(nib.load(p).dataobj).tobytes()).hexdigest()[:12]


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("work", type=Path)
    ap.add_argument("--larmorx", default="target/release/larmorx")
    ap.add_argument("--afni", default=None)
    ap.add_argument("--limit", default="4G", help="LARMORX_MEMORY_LIMIT for the default runs")
    ap.add_argument("--threads", default="2", help="OMP_NUM_THREADS (the guard's workers)")
    a = ap.parse_args()
    a.work.mkdir(parents=True, exist_ok=True)
    env = {
        "PATH": "/usr/bin:/bin",
        "HOME": str(a.work),
        "AFNI_DONT_LOGFILE": "YES",
        "AFNI_NIFTI_NOEXT": "YES",
        "OMP_NUM_THREADS": a.threads,
    }
    for name, size, units, nt in CASES:
        src = series(size, units, nt, a.work / f"{name}.nii")
        guarded = name not in ("mm0.3", "mm3")
        runs = [("original", "fix"), ("replica", "fix")]
        if not guarded:
            runs += [("original", "reproduce"), ("replica", "reproduce")]
        outs = {}
        print(f"=== {name}: voxels {size} (units {units}), {nt} volume(s)")
        for impl, mode in runs:
            d = a.work / f"{name}-{impl}-{mode}"
            d.mkdir(exist_ok=True)
            out = d / "uni.nii"
            out.unlink(missing_ok=True)
            e = dict(env, LARMORX_IMPLEMENTATION=impl, LARMORX_UPSTREAM_DEFECTS=mode)
            if mode == "fix":
                e["LARMORX_MEMORY_LIMIT"] = a.limit
            p = subprocess.run(
                [a.larmorx, "afni", "3dUnifize", "-EPI", "-prefix", str(out), str(src)],
                env=e,
                capture_output=True,
                text=True,
                timeout=600,
            )
            lines = [ln for ln in p.stderr.splitlines() if "AFNI-75" in ln or "-Urad" in ln]
            outs[(impl, mode)] = digest(out)
            print(f"  {impl:8s} {mode:9s} exit {p.returncode} output {outs[(impl, mode)]}")
            for ln in lines:
                print(f"      {ln}")
        if a.afni and not guarded:
            d = a.work / f"{name}-afni"
            d.mkdir(exist_ok=True)
            out = d / "uni.nii"
            out.unlink(missing_ok=True)
            p = subprocess.run(
                [os.path.join(a.afni, "3dUnifize"), "-EPI", "-prefix", str(out), str(src)],
                env=env,
                capture_output=True,
                text=True,
                timeout=600,
            )
            print(f"  AFNI               exit {p.returncode} output {digest(out)}")


if __name__ == "__main__":
    main()
