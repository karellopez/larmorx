# SPDX-License-Identifier: Apache-2.0
"""Black-box runs of FSL 6.0.7.23's mcflirt for specs/mcflirt.md's FSL-20 and FSL-21 (the spec
writer, 2026-10-30): for each case, the exit status (or signal), standard output and error, and
the files written. Only the installed program is run; no FSL source is needed.

    python -P validation/investigations/specs-memory/mcflirt_fsl20_21.py <out-dir> [--fsl DIR]

``--larmorx`` runs larmorx's command line instead (``python -m larmorx mri hmc``, the clean-room
original; ``LARMORX_UPSTREAM_DEFECTS`` from ``--mode``), and ``--replica BIN`` the ``larmorx-nc``
replica (``BIN fsl mcflirt``), so the three can be compared case by case. Prints one block per
case and writes ``<out-dir>/results.json``.
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
from pathlib import Path

import nibabel as nib
import numpy as np


def series(path: Path) -> Path:
    """A small smooth head-like series of 5 volumes (24 x 24 x 16 voxels of 3 mm), each volume
    shifted a little: deterministic, no random numbers."""
    shape = (24, 24, 16)
    i, j, k = np.indices(shape, dtype=np.float64)
    vols = []
    for t in range(5):
        r = np.sqrt((i - 12 - 0.3 * t) ** 2 + (j - 12) ** 2 + ((k - 8 + 0.2 * t) * 1.3) ** 2)
        vols.append(np.where(r < 9, 800.0 + 4 * i + 2 * j, 40.0) + np.where(r < 4, 300.0, 0.0))
    data = np.stack(vols, axis=-1).astype(np.float32)
    img = nib.Nifti1Image(data, np.diag([3.0, 3.0, 3.0, 1.0]))
    img.header.set_zooms((3.0, 3.0, 3.0, 2.0))
    img.header.set_xyzt_units("mm", "sec")
    nib.save(img, path)
    return path


def _c(cost: str, bins: str, *rest: str) -> list[str]:
    return ["-out", "o", "-cost", cost, "-bins", bins, *rest]


CASES = {
    # FSL-20: the .mat directory's parent does not exist.
    "fsl20-mats": ["-out", "missing/o", "-mats", "-stages", "1"],
    "fsl20-mats-plots": ["-out", "missing/o", "-mats", "-plots", "-stages", "1"],
    "fsl20-rmsrel": ["-out", "missing/o", "-rmsrel", "-stages", "1"],
    "fsl20-rmsabs-report": ["-out", "missing/o", "-rmsabs", "-report", "-stages", "1"],
    "fsl20-plots-only": ["-out", "missing/o", "-plots", "-stages", "1"],
    "fsl20-none": ["-out", "missing/o", "-stages", "1"],
    "fsl20-reffile": ["-out", "missing/o", "-mats", "-reffile", "@ref", "-stages", "1"],
    "fsl20-perm": ["-out", "ro/o", "-mats", "-stages", "1"],
    "fsl20-file-parent": ["-out", "afile/o", "-mats", "-stages", "1"],
    # The .par file cannot be opened (a directory of that name): mcflirt's other exit 0.
    "par-dir-plots-mats": ["-out", "o", "-mats", "-plots", "-rmsrel", "-stages", "1"],
    "par-dir-plots": ["-out", "o", "-plots", "-stages", "1"],
    "par-dir-plots-mats-ref0": ["-out", "o", "-mats", "-plots", "-refvol", "0", "-stages", "1"],
    # The .rms files cannot be opened: silently not written.
    "rms-dir": ["-out", "o", "-mats", "-rmsrel", "-rmsabs", "-stages", "1"],
    # FSL-21: histogram costs with fewer than 8 bins.
    "bins7-mutualinfo": _c("mutualinfo", "7", "-stages", "1", "-mats"),
    "bins4-mutualinfo-s2": _c("mutualinfo", "4", "-stages", "2", "-mats"),
    "bins4-mutualinfo-s3-plots": _c("mutualinfo", "4", "-mats", "-plots"),
    "bins0-normmi": _c("normmi", "0", "-stages", "1", "-mats"),
    "bins1-woods": _c("woods", "1", "-stages", "1", "-mats"),
    "bins2-corratio": _c("corratio", "2", "-stages", "1", "-mats"),
    "bins2-corratio-smooth0": _c("corratio", "2", "-stages", "1", "-mats", "-smooth", "0"),
    "bins2-corratio-report": _c("corratio", "2", "-stages", "1", "-mats", "-report"),
    "bins8-corratio": _c("corratio", "8", "-stages", "1", "-mats"),
    "bins8-mutualinfo-s3": _c("mutualinfo", "8", "-mats"),
    "bins2-normcorr": _c("normcorr", "2", "-stages", "1", "-mats"),
    "bins2-leastsq": _c("leastsquares", "2", "-stages", "1", "-mats"),
    "bins4-mutualinfo-s0": _c("mutualinfo", "4", "-stages", "0", "-mats"),
    "bins4-mutualinfo-ref1": [
        "-out",
        "o",
        "-cost",
        "mutualinfo",
        "-bins",
        "4",
        "-stages",
        "1",
        "-mats",
        "-reffile",
        "@ref1",
    ],
    "bins4-mutualinfo-1vol": [
        "-out",
        "o",
        "-cost",
        "mutualinfo",
        "-bins",
        "4",
        "-stages",
        "1",
        "-mats",
        "@onevol",
    ],
    # Negative -bins (atoi reads them): stage 1 has bins / 8, stages 2-4 bins / 4.
    "binsm1-normcorr": ["-out", "o", "-bins", "-1", "-stages", "1", "-mats"],
    "binsm4-normcorr-s2": ["-out", "o", "-bins", "-4", "-stages", "2", "-mats"],
    "binsm8-normcorr": ["-out", "o", "-bins", "-8", "-stages", "1", "-mats"],
    "binsm16-normcorr": ["-out", "o", "-bins", "-16", "-stages", "1", "-mats"],
    "binsm16-mutualinfo": _c("mutualinfo", "-16", "-stages", "1", "-mats"),
    "binsabc-mutualinfo": _c("mutualinfo", "abc", "-stages", "1", "-mats"),
}


#: Directories made before the run, in the names of files mcflirt writes.
PRE_DIRS = {
    "par-dir-plots-mats": ("o.par",),
    "par-dir-plots": ("o.par",),
    "par-dir-plots-mats-ref0": ("o.par",),
    "rms-dir": ("o_rel.rms", "o_abs_mean.rms"),
}


def files(d: Path) -> list[str]:
    skip = {"in.nii.gz", "ref.nii.gz", "ref1.nii.gz", "one.nii.gz", "afile"}
    return sorted(str(p.relative_to(d)) for p in d.rglob("*") if p.name not in skip)


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("out", type=Path)
    ap.add_argument("--fsl", type=Path, default=Path.home() / "fsl-6.0.7.23")
    ap.add_argument("--larmorx", action="store_true")
    ap.add_argument("--replica", type=Path)
    ap.add_argument("--mode", default="fix")
    ap.add_argument("--only", nargs="*")
    a = ap.parse_args()
    a.out.mkdir(parents=True, exist_ok=True)
    env = {
        "PATH": "/usr/bin:/bin",
        "FSLDIR": str(a.fsl),
        "FSLOUTPUTTYPE": "NIFTI_GZ",
        "LC_ALL": "C",
        "HOME": str(a.out),
        "OMP_NUM_THREADS": "1",
        "OPENBLAS_CORETYPE": "Haswell",
        "LARMORX_UPSTREAM_DEFECTS": a.mode,
    }
    if a.larmorx:
        env["PYTHONPATH"] = os.environ.get("PYTHONPATH", "")
        program = [sys.executable, "-m", "larmorx", "mri", "hmc"]
    elif a.replica:
        program = [str(a.replica), "fsl", "mcflirt"]
    else:
        program = [str(a.fsl / "bin" / "mcflirt")]
    results = {}
    for name, args in CASES.items():
        if a.only and name not in a.only:
            continue
        d = a.out / name
        d.mkdir(exist_ok=True)
        inp = series(d / "in.nii.gz")
        img = nib.load(inp)
        data = np.asarray(img.dataobj)
        nib.save(nib.Nifti1Image(data[..., 2], img.affine, img.header), d / "ref.nii.gz")
        nib.save(nib.Nifti1Image(data[..., :1], img.affine, img.header), d / "one.nii.gz")
        nib.save(nib.Nifti1Image(data[..., 2:3], img.affine, img.header), d / "ref1.nii.gz")
        (d / "afile").write_text("x", encoding="utf-8")
        subs = {"@ref": "ref.nii.gz", "@ref1": "ref1.nii.gz"}
        words = [subs.get(w, w) for w in args if w != "@onevol"]
        source = "one.nii.gz" if "@onevol" in args else str(inp)
        for blocked in PRE_DIRS.get(name, ()):
            (d / blocked).mkdir(exist_ok=True)
        if name == "fsl20-perm":
            (d / "ro").mkdir(exist_ok=True)
            os.chmod(d / "ro", 0o555)
        p = subprocess.run(
            [*program, "-in", source, *words],
            cwd=d,
            env=env,
            capture_output=True,
            text=True,
            check=False,
            timeout=600,
        )
        if name == "fsl20-perm":
            os.chmod(d / "ro", 0o755)
        mats = {
            str(m.relative_to(d)): m.read_text(encoding="utf-8") for m in sorted(d.rglob("MAT_*"))
        }
        par = {
            str(m.relative_to(d)): m.read_text(encoding="utf-8")
            for m in sorted(d.rglob("*.par"))
            if m.is_file()
        }
        results[name] = {
            "args": args,
            "returncode": p.returncode,
            "stdout": p.stdout[-3000:],
            "stderr": p.stderr[-3000:],
            "files": files(d),
            "mats": mats,
            "par": par,
        }
        print(f"== {name}: {' '.join(args)}\n   exit {p.returncode}; files {files(d)}")
        for line in p.stderr.splitlines()[-8:]:
            print(f"   err| {line}")
        for line in p.stdout.splitlines()[-3:]:
            print(f"   out| {line}")
    (a.out / "results.json").write_text(json.dumps(results, indent=1), encoding="utf-8")


if __name__ == "__main__":
    main()
