# SPDX-License-Identifier: Apache-2.0
"""FSL-67's measurement (rule D): does computing BBR's boundary normals in millimetres (the
segmentation's voxel sizes) instead of FLIRT's (the reference level's) change the registration,
and is the change an improvement?

1. fMRIPrep's boundary-based calls on the record's three oracle runs: the replica in its default
   mode (the normals in millimetres) against ``reproduce`` (FLIRT's; checked to be fMRIPrep's own
   matrix byte for byte): the displacement between the two matrices over the T1w's brain.
2. A known truth: ds005040's masked T1w, every third slice (0.85 × 0.83 × 2.5 mm: a strongly
   anisotropic reference and segmentation), and the same T1w moved by a known rigid motion and
   resampled to 2 mm as the input; BBR from three perturbed starts, both modes, each result's
   displacement from the true matrix over the brain.

Needs the replica (``LARMORX_NC_BIN``, else the checkout's ``crates-nc/target/release``) built at a
commit whose default mode corrects FSL-67, FSL 6.0.7.23's ``bbr.sch``, and the fMRIPrep oracle
runs. Writes ``<out>/fsl67.json`` and prints a table.

    PYTHONPATH=<checkout>/python:<checkout>/validation/src python -P fsl67_measure.py <out> [jobs]
"""

from __future__ import annotations

import json
import os
import shutil
import subprocess
import sys
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

import nibabel as nib
import numpy as np
from larmorx_validation.evidence import fsl_flirt as ev
from larmorx_validation.parity import fsl_flirt_replica as fp
from scipy import ndimage

SLOPE = "0.5"  # a T1-weighted input: white matter brighter than grey (FLIRT's -bbrslope)


def replica(args: list[str], cwd: Path, mode: str, inputs: dict[str, Path]) -> np.ndarray:
    cwd.mkdir(parents=True, exist_ok=True)
    for name, src in inputs.items():
        shutil.copy(src, cwd / name)
    env = {
        "HOME": os.environ.get("HOME", "/tmp"),
        "PATH": "/usr/bin:/bin",
        "FSLOUTPUTTYPE": "NIFTI_GZ",
        "LARMORX_UPSTREAM_DEFECTS": mode,
        "OMP_NUM_THREADS": "4",
    }
    exe = fp.replica_binary()
    p = subprocess.run(
        [str(exe), "fsl", "flirt", *args], cwd=cwd, env=env, capture_output=True, text=True
    )
    if p.returncode != 0:
        raise RuntimeError(f"{cwd}: exit {p.returncode}: {p.stderr[-500:]}")
    (cwd / "stderr.txt").write_text(p.stderr, encoding="utf-8")
    return np.loadtxt(cwd / "out.mat")


def disp(a: np.ndarray, b: np.ndarray, ref, brain: np.ndarray) -> dict:
    d = ev.displacement(a, b, ref, brain)
    return {k: d[k] for k in ("mean", "p95", "max")}


def fmriprep_part(out: Path, jobs: int) -> list[dict]:
    todo = []
    for run in fp.STANDARD_RUNS:
        node = ev._bbr_node(run)
        args = fp.fmriprep_args(run, node)
        i = args.index("-omat")
        args[i + 1] = "out.mat"
        if "-out" in args:
            j = args.index("-out")
            del args[j : j + 2]
        todo.append((run, node, args))

    def one(t):
        run, node, args = t
        fix = replica(args, out / run / "fix", "fix", {})
        rep = replica(args, out / run / "reproduce", "reproduce", {})
        ref_path = Path(args[args.index("-ref") + 1])
        ref = nib.load(str(ref_path))
        brain = np.asanyarray(ref.dataobj) > 0
        own = np.loadtxt(next(node.glob("*.mat")))
        seg = nib.load(args[args.index("-wmseg") + 1])
        return {
            "run": run,
            "segmentation voxels (mm)": [float(z) for z in seg.header.get_zooms()[:3]],
            "reproduce is fMRIPrep's own": bool(np.array_equal(rep, own)),
            "fix - reproduce (mm)": disp(fix, rep, ref, brain),
            "rms deviation (80 mm)": ev.rms80(fix, rep),
        }

    with ThreadPoolExecutor(jobs) as ex:
        return list(ex.map(one, todo))


def rigid(rx: float, ry: float, rz: float, t: tuple[float, float, float]) -> np.ndarray:
    a, b, c = np.deg2rad([rx, ry, rz])
    rxm = np.array([[1, 0, 0], [0, np.cos(a), -np.sin(a)], [0, np.sin(a), np.cos(a)]])
    rym = np.array([[np.cos(b), 0, np.sin(b)], [0, 1, 0], [-np.sin(b), 0, np.cos(b)]])
    rzm = np.array([[np.cos(c), -np.sin(c), 0], [np.sin(c), np.cos(c), 0], [0, 0, 1]])
    m = np.eye(4)
    m[:3, :3] = rxm @ rym @ rzm
    m[:3, 3] = t
    return m


def synthetic_part(out: Path, jobs: int) -> dict:
    src = fp.inputs()
    t1 = nib.load(str(src / "t1full.nii.gz"))
    node = ev._bbr_node(fp.SOURCE_RUN)
    wmimg = nib.load(
        fp.fmriprep_args(fp.SOURCE_RUN, node)[
            fp.fmriprep_args(fp.SOURCE_RUN, node).index("-wmseg") + 1
        ]
    )
    d = out / "synthetic"
    d.mkdir(parents=True, exist_ok=True)
    data = np.asanyarray(t1.dataobj).astype(np.float32)
    # The reference and its segmentation: every third slice along the last axis.
    ref_aff = t1.affine.copy()
    ref_aff[:3, 2] *= 3
    ref = nib.Nifti1Image(data[:, :, ::3], ref_aff)
    wm = nib.Nifti1Image((np.asanyarray(wmimg.dataobj)[:, :, ::3] > 0.5).astype(np.uint8), ref_aff)
    for img in (ref, wm):
        img.set_qform(ref_aff, 1)
        img.set_sform(ref_aff, 1)
    nib.save(ref, str(d / "ref.nii.gz"))
    nib.save(wm, str(d / "wm.nii.gz"))
    # The input: the T1w moved by W (world mm), on a 2 mm grid around the same field of view.
    w = rigid(2.0, -3.0, 1.5, (1.5, -2.0, 1.0))
    shape = tuple(
        int(np.ceil(n * z / 2.0)) for n, z in zip(t1.shape, t1.header.get_zooms(), strict=False)
    )
    in_aff = t1.affine.copy()
    for k in range(3):
        in_aff[:3, k] = t1.affine[:3, k] / np.linalg.norm(t1.affine[:3, k]) * 2.0
    # input(v) = T1(A_t1^-1 W^-1 A_in v)
    m = np.linalg.inv(t1.affine) @ np.linalg.inv(w) @ in_aff
    moved = ndimage.affine_transform(data, m[:3, :3], m[:3, 3], output_shape=shape, order=1)
    inp = nib.Nifti1Image(moved.astype(np.float32), in_aff)
    inp.set_qform(in_aff, 1)
    inp.set_sform(in_aff, 1)
    nib.save(inp, str(d / "in.nii.gz"))
    f_in, _ = ev.fsl_coords(inp)
    f_ref, _ = ev.fsl_coords(ref)
    truth = f_ref @ np.linalg.inv(ref_aff) @ np.linalg.inv(w) @ in_aff @ np.linalg.inv(f_in)
    brain = np.asanyarray(ref.dataobj) > 0
    sched = str(fp.fsldir() / "etc" / "flirtsch" / "bbr.sch")
    starts = {
        "2 deg, 2 mm": rigid(2.0, 0.0, -2.0, (2.0, 0.0, -1.0)),
        "4 deg, 3 mm": rigid(-3.0, 4.0, 0.0, (0.0, 3.0, 1.5)),
        "1 deg, 1 mm": rigid(0.0, -1.0, 1.0, (-1.0, 1.0, 0.0)),
    }
    todo = []
    for name, p in starts.items():
        init = p @ truth
        sub = d / name.replace(" ", "").replace(",", "_")
        sub.mkdir(exist_ok=True)
        np.savetxt(sub / "init.mat", init, fmt="%.10f")
        for mode in ("fix", "reproduce"):
            todo.append((name, mode, sub))

    def one(t):
        name, mode, sub = t
        args = ["-in", "in.nii.gz", "-ref", "ref.nii.gz", "-wmseg", "wm.nii.gz", "-cost", "bbr",
                "-dof", "6", "-init", "init.mat", "-schedule", sched, "-bbrslope", SLOPE,
                "-omat", "out.mat"]  # fmt: skip
        inputs = {
            "in.nii.gz": d / "in.nii.gz",
            "ref.nii.gz": d / "ref.nii.gz",
            "wm.nii.gz": d / "wm.nii.gz",
            "init.mat": sub / "init.mat",
        }
        m = replica(args, sub / mode, mode, inputs)
        return name, mode, m

    with ThreadPoolExecutor(jobs) as ex:
        res = list(ex.map(one, todo))
    rows = {}
    for name, mode, m in res:
        r = rows.setdefault(
            name,
            {
                "start - truth (mm)": disp(
                    np.loadtxt(d / name.replace(" ", "").replace(",", "_") / "init.mat"),
                    truth,
                    ref,
                    brain,
                )
            },
        )
        r[f"{mode} - truth (mm)"] = disp(m, truth, ref, brain)
    for name, r in rows.items():
        fix = next(m for n, mo, m in res if n == name and mo == "fix")
        rep = next(m for n, mo, m in res if n == name and mo == "reproduce")
        r["fix - reproduce (mm)"] = disp(fix, rep, ref, brain)
    return {
        "reference voxels (mm)": [float(z) for z in ref.header.get_zooms()[:3]],
        "true motion": "rotations 2, -3, 1.5 degrees about x, y, z; translation 1.5, -2, 1 mm",
        "starts": rows,
    }


def main() -> None:
    out = Path(sys.argv[1])
    jobs = int(sys.argv[2]) if len(sys.argv) > 2 else 6
    out.mkdir(parents=True, exist_ok=True)
    res = {"fmriprep": fmriprep_part(out, jobs), "synthetic": synthetic_part(out, jobs)}
    (out / "fsl67.json").write_text(json.dumps(res, indent=1) + "\n", encoding="utf-8")
    print(json.dumps(res, indent=1))


if __name__ == "__main__":
    main()
