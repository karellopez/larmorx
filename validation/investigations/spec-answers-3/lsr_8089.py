# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
"""specs/topup.md §7.4 (spec writer, 2026-11-03) on random testing's reproducer 8089: applytopup
`--method=lsr` (the default) on lines of 16 voxels along x, a field with a gradient along the
line, images of 16 x 24 x 14 x 2 moved by topup's estimated movements, `-n trilinear`.

`run <work>` makes the case's inputs and runs, every FSL run through the oracle cache (use mode):
FSL's topup source (`tu_fieldcoef`, `tu_movpar.txt`), FSL's applytopup output, the replica's
`reproduce` run (the `larmorx-nc` program as a black box), and two FSL `--method=jac` runs with a
zero field, one per image, which give FSL's moved images (§7.4's first step, the same
interpolation and mask) for the model.

`check <work> [--variant circular | overlap]`: the model of §7.4's text on FSL's moved images,
in FSL's voxel order (images stored neurologically are read flipped along x, the coefficients
are in that order): per line along x (the phase encoding), skipped when a voxel of the line is
outside any image's mask, `K` stacked from both images' matrices built row by row (FSL's walk;
`lsr_model.k_matrix`), `f = (KᵀK + 0.01 SᵀS)⁻¹ Kᵀ y`, every volume alike; against FSL's output
and the replica's.
"""

import json
import shutil
import sys
from pathlib import Path

import nibabel as nib
import numpy as np

sys.path.insert(0, str(Path(__file__).parent))
import lsr_model as L  # noqa: E402

FINDING = "oracles/difftest/2026-11-03-topup-original/findings/fsl-topup-original/topup-trial-08089-original.json"


def run(work: Path) -> None:
    from larmorx_validation.difftest.tools import fsl_topup as dt
    from larmorx_validation.parity import defects
    from larmorx_validation.parity import fsl_topup_replica as rep
    from larmorx_validation.parity.fsl_mcflirt_replica import workspace

    doc = json.loads((workspace() / FINDING).read_text())
    topup, at = dt.case(doc["recipe"], "replica").payload[1]
    s, src = work / "source", work / "src"
    for d in (s, src):
        shutil.rmtree(d, ignore_errors=True)
        d.mkdir(parents=True)
    r = rep._oracle(topup, rep._prepare(topup, s, None), s, dict(topup.settings))
    print("FSL topup (source) exit", r.code)
    for f in s.iterdir():
        if f.name.startswith(("tu_fieldcoef", "tu_movpar.txt")):
            shutil.copy(f, src / f.name)
    import dataclasses

    at = dataclasses.replace(at, source_dir=str(src))
    for who in ("fsl", "replica"):
        d = work / who
        shutil.rmtree(d, ignore_errors=True)
        d.mkdir(parents=True)
        args = rep._prepare(at, d, None)
        if who == "fsl":
            p = rep._oracle(at, args, d, dict(at.settings))
        else:
            p = rep.run_replica(
                "applytopup", args, cwd=d, settings=dict(at.settings), mode=defects.REPRODUCE
            )
        (d / "ARGS.txt").write_text(" ".join(args) + "\n")
        (d / "STDERR.txt").write_text(p.stderr)
        print(who, "applytopup exit", p.code)
    # FSL's moved images: --method=jac with a zero field, one image at a time
    args = (work / "fsl" / "ARGS.txt").read_text().split()
    imain = next(args[i + 1] for i, a in enumerate(args) if a == "-i").split(",")
    index = next(args[i + 1] for i, a in enumerate(args) if a == "-x").split(",")
    acq = next(args[i + 1] for i, a in enumerate(args) if a == "-a")
    for k, (im, ix) in enumerate(zip(imain, index)):
        d = work / f"moved{k}"
        shutil.rmtree(d, ignore_errors=True)
        d.mkdir(parents=True)
        for f in (im, acq):
            shutil.copy(work / "fsl" / f, d / Path(f).name)
        shutil.copy(src / "tu_movpar.txt", d / "zero_movpar.txt")
        c = nib.load(str(next(src.glob("tu_fieldcoef*"))))
        nib.save(
            nib.Nifti1Image(np.zeros(c.shape, np.float32), None, c.header),
            str(d / "zero_fieldcoef.nii.gz"),
        )
        jac = [
            "-i",
            Path(im).name,
            "-a",
            Path(acq).name,
            "-x",
            ix,
            "-t",
            "zero",
            "-o",
            "moved",
            "-n",
            "trilinear",
            "--method=jac",
        ]
        run_at = dataclasses.replace(at, args=tuple(jac), source_dir="")
        p = rep._oracle(run_at, jac, d, dict(at.settings))
        print(f"moved{k} (jac, zero field) exit", p.code)


def check(work: Path, variant: str) -> None:
    from scipy import ndimage  # noqa: F401  (not needed: FSL moves the images)

    args = (work / "fsl" / "ARGS.txt").read_text().split()
    imain = next(args[i + 1] for i, a in enumerate(args) if a == "-i").split(",")
    index = [int(v) for v in next(args[i + 1] for i, a in enumerate(args) if a == "-x").split(",")]
    acq = np.loadtxt(work / "fsl" / next(args[i + 1] for i, a in enumerate(args) if a == "-a"))
    img0 = nib.load(str(work / "fsl" / imain[0]))
    flip = np.linalg.det(img0.affine[:3, :3]) > 0  # stored neurologically: FSL reads x flipped
    order = (lambda a: a[::-1]) if flip else (lambda a: a)
    moved = []
    for k in range(len(imain)):
        m = np.asarray(nib.load(str(work / f"moved{k}" / "moved.nii.gz")).dataobj, np.float64)
        moved.append(order(m))
    want = order(np.asarray(nib.load(str(work / "fsl" / "corrected.nii.gz")).dataobj, np.float64))
    repl = order(
        np.asarray(nib.load(str(work / "replica" / "corrected.nii.gz")).dataobj, np.float64)
    )
    c = nib.load(str(next((work / "src").glob("tu_fieldcoef*"))))
    k = [round(float(v)) for v in c.header.get_zooms()[:3]]
    nf = [round(float(v)) for v in c.get_qform()[:3, 3]]
    coef = np.asarray(c.dataobj, np.float64)
    # the field on the image grid (§4.2's layout), held in single precision
    h = coef
    for a in range(3):
        centres = np.arange(coef.shape[a], dtype=float)
        centres = centres if k[a] == 1 else (centres - 1.0) * k[a]
        u = (np.arange(nf[a], dtype=float)[:, None] - centres[None, :]) / k[a]
        h = np.moveaxis(np.tensordot(L.bspline3(u), np.moveaxis(h, a, 0), axes=(1, 0)), 0, a)
    h = h.astype(np.float32).astype(np.float64)
    nx, ny, nz, nt = want.shape
    mask = np.ones((nx, ny, nz), bool)
    for m in moved:
        mask &= np.all(m != 0, axis=3)
    S = L.s_matrix(nx)
    out = np.zeros_like(want)
    build = L.overlap_matrix if variant == "overlap" else L.k_matrix
    for z in range(nz):
        for y in range(ny):
            if not mask[:, y, z].all():
                continue
            hl = h[:, y, z]
            K = np.vstack([build(hl, acq[ix - 1, 0] * acq[ix - 1, 3], variant) for ix in index])
            A = K.T @ K + 0.01 * S.T @ S
            for t in range(nt):
                yv = np.concatenate([m[:, y, z, t] for m in moved])
                out[:, y, z, t] = np.linalg.solve(A, K.T @ yv)
    brain = want != 0
    for name, ref in (("FSL", want), ("the replica (reproduce)", repl)):
        e = np.abs(out - ref)[brain] / np.abs(ref[brain])
        print(
            f"{variant}: the model against {name}: relative p50 {np.percentile(e, 50):.3g}, p99 "
            f"{np.percentile(e, 99):.3g}, max {e.max():.3g}; zeros {int((out == 0).sum())} vs "
            f"{int((ref == 0).sum())}"
        )
    print(f"the replica (reproduce) against FSL: max |d| {np.abs(repl - want).max():.3g}")


def main():
    work = Path(sys.argv[2])
    if sys.argv[1] == "run":
        run(work)
    else:
        v = sys.argv[sys.argv.index("--variant") + 1] if "--variant" in sys.argv else "fsl"
        check(work, v)


main()
