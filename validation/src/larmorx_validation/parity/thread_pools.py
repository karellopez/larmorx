# SPDX-License-Identifier: Apache-2.0
"""Does a call of a larmorx program use only its own pool of the call's thread count?

larmorx runs a call's parallel work in a pool of exactly the call's size
(``larmorx_core::parallel::with_threads``, threads named ``larmorx-<n>-<i>``; CLAUDE.md rule 5,
explicit ``n_threads``). Work reached outside such a pool runs on rayon's global pool, whose size
is the CPUs the process may use (or ``RAYON_NUM_THREADS``), whatever the call asked for: more
threads than a pipeline gave the program (nipype's ``NSLOTS``), never a different result. Found
in the data panel's re-validation (2026-10-26, ``docs/findings/ants-cli.md``).

:func:`programs` lists every registered program (``larmorx --help``: ANTs, AFNI, FSL, mri) over
its distinct code paths (each registration stage and metric, warps, the filters, Atropos'
initialisations), the AFNI originals in-process and the GPL and FSL replicas as their own
programs when built; :func:`probe` runs one at one thread (``ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS
=1`` for the ANTs programs, ``OMP_NUM_THREADS=1`` for the others) on small seeded inputs
(:func:`inputs`) and lists every thread of its processes from ``/proc/<pid>/task`` every 10 ms:
any thread but a process's main thread and its ``larmorx-<n>`` pools is work outside the call's
pool. Linux only. ``tests/parity/test_thread_pools.py`` runs every case;
``validation/investigations/panel-ants/global_pool_probe.py`` prints the table.
"""

from __future__ import annotations

import os
import subprocess
import sys
import time
from dataclasses import dataclass
from pathlib import Path

import numpy as np


@dataclass(frozen=True)
class Program:
    """One program and code path: its command line and environment."""

    name: str
    argv: tuple[str, ...]
    env: tuple[tuple[str, str], ...]


def threads(pid: int) -> dict[int, str]:
    out = {}
    for t in Path(f"/proc/{pid}/task").glob("*"):
        try:
            out[int(t.name)] = (t / "comm").read_text().strip()
        except (OSError, ValueError):
            continue
    return out


def descendants(pid: int) -> list[int]:
    """``pid`` and the processes it started (a wrapper running a replica program)."""
    out, todo = [], [pid]
    while todo:
        p = todo.pop()
        out.append(p)
        for t in Path(f"/proc/{p}/task").glob("*"):
            try:
                todo += [int(c) for c in (t / "children").read_text().split()]
            except (OSError, ValueError):
                continue
    return out


def probe(p: Program, cwd: Path) -> tuple[int, int, dict[str, int], str]:
    """(exit status, ``larmorx-<n>`` pool threads, the other threads by name, the end of
    standard error) of every process of the run, each process's main thread aside."""
    proc = subprocess.Popen(
        list(p.argv),
        env=dict(p.env),
        cwd=cwd,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.PIPE,
        text=True,
    )
    seen: dict[tuple[int, int], str] = {}
    while proc.poll() is None:
        for pid in descendants(proc.pid):
            for tid, name in threads(pid).items():
                if tid != pid:
                    seen[(pid, tid)] = name
        time.sleep(0.01)
    err = proc.stderr.read() if proc.stderr else ""
    pool = sum(1 for n in seen.values() if n.startswith("larmorx-"))
    other: dict[str, int] = {}
    for n in seen.values():
        if not n.startswith("larmorx-"):
            other[n] = other.get(n, 0) + 1
    return proc.returncode, pool, other, err[-400:]


def inputs(d: Path) -> dict[str, str]:
    """Small seeded inputs: two heads 2 voxels apart, a mask, labels, three priors, an
    8-volume series, a T1-like head with a bias, a smooth displacement field."""
    import nibabel as nib

    rng = np.random.default_rng(7)
    shape = (48, 48, 36)
    i, j, k = np.indices(shape, dtype=np.float64)
    r = np.sqrt((i - 24) ** 2 + (j - 24) ** 2 + ((k - 18) * 1.3) ** 2)
    head = np.where(r < 18, 400 + 3 * i, 20) + np.where(r < 9, 300, 0)
    img = (head + rng.normal(0, 10, shape)).astype(np.float32)
    a = np.diag([2.0, 2.0, 2.0, 1.0])
    out = {}

    def save(name: str, data, zooms=None, intent=None) -> None:
        im = nib.Nifti1Image(data, a)
        if zooms is not None:
            im.header.set_zooms(zooms)
        if intent is not None:
            im.header.set_intent(intent)
        nib.save(im, d / name)
        out[name.split(".")[0]] = str(d / name)

    save("fixed.nii.gz", img)
    save("moving.nii.gz", np.roll(img, 2, axis=0))
    save("mask.nii.gz", (r < 18).astype(np.uint8))
    save("labels.nii.gz", ((r < 18).astype(np.uint8) + (r < 9).astype(np.uint8)))
    inside = r < 18
    for c, sel in enumerate((inside & (r >= 9), r < 9, inside & (k > 18)), 1):
        save(f"prior{c:02d}.nii.gz", sel.astype(np.float32))
    series = np.stack([np.roll(img, t % 2, axis=1) * (1 + 0.01 * t) for t in range(8)], -1)
    save("series.nii.gz", series.astype(np.float32), zooms=(2.0, 2.0, 2.0, 2.0))
    t1 = (np.where(r < 18, 300, 30) + np.where(r < 9, 150, 0) + rng.normal(0, 5, shape)) * (
        1 + 0.01 * i
    )
    save("t1.nii.gz", t1.astype(np.float32))
    field = np.zeros((*shape, 1, 3), np.float32)
    field[..., 0, 0] = 0.5 * (i / shape[0]) - 0.25
    field[..., 0, 1] = 0.25 * (k / shape[2])
    save("warp.nii.gz", field, intent="vector")
    return out


#: The inputs :func:`inputs` writes.
INPUTS = ("fixed", "moving", "mask", "labels", "prior01", "prior02", "prior03", "series", "t1",
          "warp")  # fmt: skip


def programs(
    d: Path, gpl: str | None = None, nc: str | None = None, *, make: bool = True
) -> list[Program]:
    """Every registered program over its distinct code paths, on :func:`inputs` in ``d``
    (``make=False``: the list only, the inputs not written)."""
    x = inputs(d) if make else {k: str(d / f"{k}.nii.gz") for k in INPUTS}
    f, m, mask, lab = x["fixed"], x["moving"], x["mask"], x["labels"]
    s, t1 = x["series"], x["t1"]
    o = str(d / "o")
    lx = (sys.executable, "-m", "larmorx")
    base = {k: v for k, v in os.environ.items() if k not in ("RAYON_NUM_THREADS", "NSLOTS")}
    itk = tuple({**base, "ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS": "1",
                 "ANTS_RANDOM_SEED": "42"}.items())  # fmt: skip
    omp = tuple({**base, "OMP_NUM_THREADS": "1", "FSLOUTPUTTYPE": "NIFTI_GZ"}.items())
    out: list[Program] = []

    def reg(tag: str, *stage: str) -> list[str]:
        return ["-d", "3", "-o", f"[{o}_{tag},{o}_{tag}_w.nii.gz]", *stage,
                "-c", "[4x2,1e-6,5]", "-f", "2x1", "-s", "1x0vox"]  # fmt: skip

    def points(tag: str, metric: str) -> list[str]:
        # A point-set metric: the fixed mask gives the virtual domain; no warped images.
        return ["-d", "3", "-o", f"{o}_{tag}", "-x", mask, "-t", "Affine[0.1]", "-m", metric,
                "-c", "[4x2,1e-6,5]", "-f", "2x1", "-s", "1x0vox"]  # fmt: skip

    mi, cc, ms = f"MI[{f},{m},1,32,Regular,0.25]", f"CC[{f},{m},1,2]", f"MeanSquares[{f},{m},1]"
    ants = {
        "antsAI": ["-d", "3", "-m", f"Mattes[{f},{m},32,Regular,0.25]", "-t", "Affine[0.1]",
                   "-s", "[20,0.12]", "-o", o + "_ai.mat"],
        "antsAffineInitializer": ["3", f, m, o + "_init.mat", "15", "0.1", "0", "10"],
        "antsRegistration Rigid MI": reg("rmi", "-t", "Rigid[0.1]", "-m", mi),
        "antsRegistration Affine Mattes": reg("ama", "-t", "Affine[0.1]", "-m",
                                              f"Mattes[{f},{m},1,32,Random,0.3]"),
        "antsRegistration Rigid CC": reg("rcc", "-t", "Rigid[0.1]", "-m", cc),
        "antsRegistration Rigid MeanSquares": reg("rms", "-t", "Rigid[0.1]", "-m", ms),
        "antsRegistration Rigid GC": reg("rgc", "-t", "Rigid[0.1]", "-m",
                                         f"GC[{f},{m},1,1,Regular,0.5]"),
        "antsRegistration SyN CC": reg("syn", "-t", "SyN[0.1,3,0]", "-m", cc),
        "antsRegistration BSplineSyN": reg("bsyn", "-t", "BSplineSyN[0.1,26,0,3]", "-m", cc),
        "antsRegistration GaussianDisplacementField": reg(
            "gdf", "-t", "GaussianDisplacementField[0.1,3,0]", "-m", cc),
        "antsRegistration BSplineDisplacementField": reg(
            "bdf", "-t", "BSplineDisplacementField[0.1,26,0,3]", "-m", cc),
        "antsRegistration BSpline": reg("bsp", "-t", "BSpline[0.1,4]", "-m", ms),
        "antsRegistration Exponential": reg("exp", "-t", "Exponential[0.25,3,0.5]", "-m", cc),
        "antsRegistration BSplineExponential": reg(
            "bexp", "-t", "BSplineExponential[0.25,26,0,0,3]", "-m", cc),
        "antsRegistration TimeVaryingVelocityField": reg(
            "tvf", "-t", "TimeVaryingVelocityField[0.5,4,3,0,0,0]", "-m", cc),
        "antsRegistration TimeVaryingBSplineVelocityField": reg(
            "tvb", "-t", "TimeVaryingBSplineVelocityField[0.5,4x4x4x2,4,3]", "-m", cc),
        "antsApplyTransforms warp Lanczos": ["-d", "3", "-i", m, "-r", f, "-t", x["warp"],
                                             "-n", "LanczosWindowedSinc", "-o", o + "_at.nii.gz"],
        "antsApplyTransforms -e 3 BSpline": ["-d", "3", "-e", "3", "-i", s, "-r", f,
                                             "-t", "identity", "-n", "BSpline",
                                             "-o", o + "_at4.nii.gz"],
        "Atropos KMeans MRF": ["-d", "3", "-a", f, "-x", mask, "-i", "KMeans[3]", "-c", "[3,0]",
                               "-m", "[0.1,1x1x1]", "-o", f"[{o}_seg.nii.gz,{o}_post%02d.nii.gz]"],
        "Atropos two channels": ["-d", "3", "-a", f, "-a", t1, "-x", mask, "-i", "KMeans[3]",
                                 "-c", "[3,0]", "-m", "[0.1,1x1x1]",
                                 "-o", f"[{o}_seg3.nii.gz,{o}_q%02d.nii.gz]"],
        "Atropos priors": ["-d", "3", "-a", f, "-x", mask, "-i",
                           f"PriorProbabilityImages[3,{d}/prior%02d.nii.gz,0.25]", "-c", "[3,0]",
                           "-m", "[0.1,1x1x1]", "-o", f"[{o}_seg2.nii.gz,{o}_p%02d.nii.gz]"],
        "N4BiasFieldCorrection": ["-d", "3", "-i", f, "-x", mask, "-o", o + "_n4.nii.gz",
                                  "-s", "2", "-c", "[10x10,0]"],
        "DenoiseImage": ["-d", "3", "-i", f, "-x", mask, "-o", o + "_dn.nii.gz"],
        "ImageMath G": ["3", o + "_g.nii.gz", "G", f, "1.5"],
        "ImageMath Laplacian": ["3", o + "_l.nii.gz", "Laplacian", f, "1.5", "1"],
        "ImageMath TruncateImageIntensity": ["3", o + "_t.nii.gz", "TruncateImageIntensity", f,
                                             "0.01", "0.999", "256"],
        "ImageMath MD": ["3", o + "_md.nii.gz", "MD", mask, "2"],
        "ImageMath FillHoles": ["3", o + "_fh.nii.gz", "FillHoles", mask, "2"],
        "ImageMath GetLargestComponent": ["3", o + "_gl.nii.gz", "GetLargestComponent", mask],
        "ImageMath D": ["3", o + "_d.nii.gz", "D", mask],
        "ImageMath PadImage": ["3", o + "_p.nii.gz", "PadImage", lab, "5"],
        "ImageMath m": ["3", o + "_m.nii.gz", "m", f, m],
        "ImageMath Neg": ["3", o + "_neg.nii.gz", "Neg", mask],
        "ImageMath Normalize": ["3", o + "_nz.nii.gz", "Normalize", f],
        "ImageMath RescaleImage": ["3", o + "_ri.nii.gz", "RescaleImage", f, "0", "1"],
        "ImageMath ThresholdAtMean": ["3", o + "_tm.nii.gz", "ThresholdAtMean", f, "0.5"],
        "ImageMath ReplaceVoxelValue": ["3", o + "_rv.nii.gz", "ReplaceVoxelValue", lab, "1",
                                        "1", "5"],
        "ImageMath MaurerDistance": ["3", o + "_mdt.nii.gz", "MaurerDistance", mask, "1"],
        "ImageMath ExtractContours": ["3", o + "_ec.nii.gz", "ExtractContours", lab],
        "ImageMath Grad": ["3", o + "_gr.nii.gz", "Grad", f, "1", "1"],
        "ImageMath UnsharpMask": ["3", o + "_um.nii.gz", "UnsharpMask", f, "0.5", "1"],
        "ImageMath ME": ["3", o + "_me.nii.gz", "ME", mask, "2"],
        "ImageMath GO": ["3", o + "_go.nii.gz", "GO", f, "1"],
        "ThresholdImage Otsu": ["3", f, o + "_th.nii.gz", "Otsu", "3"],
        "MultiplyImages": ["3", f, mask, o + "_mu.nii.gz"],
        "SmoothImage Gaussian": ["3", f, "1.5", o + "_sg.nii.gz"],
        "SmoothImage median": ["3", f, "1", o + "_sm.nii.gz", "0", "1"],
        "ResampleImage B-spline": ["3", f, o + "_rb.nii.gz", "1.5", "0", "4"],
        "ResampleImage sinc": ["3", f, o + "_rs.nii.gz", "1.5", "0", "3"],
        "ResampleImageBySpacing": ["3", f, o + "_rbs.nii.gz", "3", "3", "3"],
        # The other paths: point-set metrics, Demons, masks with an image-based initial
        # transform and a composite output, field and label outputs, antsAI's blobs and
        # principal axes, Atropos' other initialisations and likelihood, N4's weights,
        # DenoiseImage's Gaussian model with its noise image.
        "antsRegistration Affine ICP": points("icp", f"ICP[{lab},{lab},1]"),
        "antsRegistration Affine PSE": points("pse", f"PSE[{lab},{lab},1,1,1,1,8]"),
        "antsRegistration Affine JHCT": points("jhct", f"JHCT[{lab},{lab},1]"),
        "antsRegistration SyN Demons": reg("dem", "-t", "SyN[0.1,3,0]", "-m",
                                           f"Demons[{f},{m},1]"),
        "antsRegistration masks, initial moving transform, composite": [
            "-d", "3", "-o", f"[{o}_mx]", "-r", f"[{f},{m},1]", "-x", f"[{mask},{mask}]",
            "--write-composite-transform", "1", "-t", "Rigid[0.1]", "-m", mi,
            "-c", "[4x2,1e-6,5]", "-f", "2x1", "-s", "1x0vox"],
        "antsApplyTransforms composite field output": [
            "-d", "3", "-i", m, "-r", f, "-t", x["warp"], "-t", "identity",
            "-o", f"[{o}_cf.nii.gz,1]"],
        "antsApplyTransforms GenericLabel": ["-d", "3", "-i", lab, "-r", f, "-t", x["warp"],
                                             "-n", "GenericLabel", "-o", o + "_gl2.nii.gz"],
        "antsAI masks principal axes": ["-d", "3", "-m", f"Mattes[{f},{m},32,Regular,0.25]",
                                        "-t", "Rigid[0.1]", "-p", "1", "-x", mask,
                                        "-s", "[20,0.12]", "-o", o + "_aip.mat"],
        "antsAI blobs": ["-d", "3", "-m", f"Mattes[{f},{m},32,Regular,0.25]",
                         "-t", "Rigid[0.1]", "-b", "[10,5]", "-s", "[20,0.12]",
                         "-o", o + "_aib.mat"],
        "Atropos Otsu Parzen": ["-d", "3", "-a", f, "-x", mask, "-i", "Otsu[3]",
                                "-k", "HistogramParzenWindows[1,32]", "-c", "[3,0]",
                                "-m", "[0.1,1x1x1]", "-o", f"[{o}_seg4.nii.gz]"],
        "Atropos PriorLabelImage": ["-d", "3", "-a", f, "-x", mask,
                                    "-i", f"PriorLabelImage[2,{lab},0.5]", "-c", "[3,0]",
                                    "-m", "[0.1,1x1x1]", "-o", f"[{o}_seg5.nii.gz]"],
        "N4BiasFieldCorrection weights, rescaled": [
            "-d", "3", "-i", f, "-x", mask, "-w", mask, "-r", "1",
            "-o", f"[{o}_n4w.nii.gz,{o}_n4b.nii.gz]", "-s", "2", "-c", "[10x10,0]"],
        "DenoiseImage Gaussian with noise": ["-d", "3", "-i", f, "-n", "Gaussian",
                                             "-o", f"[{o}_dg.nii.gz,{o}_dgn.nii.gz]"],
    }  # fmt: skip
    # antsAI's blobs on these heads find no matched pair: ANTs' own result (ANTS-114's NaNs)
    # instead of larmorx's error, so that the path runs to its end.
    reproduce = (*itk, ("LARMORX_UPSTREAM_DEFECTS", "reproduce"))
    for name, a in ants.items():
        env = reproduce if name == "antsAI blobs" else itk
        out.append(Program(f"ants {name}", (*lx, "ants", name.split(" ")[0], *a), env))
    afni = {
        "3dTshift": ["-tpattern", "alt+z", "-prefix", o + "_ts.nii.gz", s],
        "3dAutomask": ["-prefix", o + "_am.nii.gz", s],
        "3dUnifize": ["-prefix", o + "_un.nii.gz", "-input", t1],
    }
    for name, a in afni.items():
        env = (*omp, ("LARMORX_IMPLEMENTATION", "original"))
        out.append(Program(f"afni {name} (clean-room original)", (*lx, "afni", name, *a), env))
        if gpl:
            a2 = [w.replace(o, o + "_gpl") for w in a]
            out.append(
                Program(f"afni {name} (replica, larmorx-gpl)", (gpl, "afni", name, *a2), omp)
            )
    out.append(Program("mri hmc (clean-room original)",
                       (*lx, "mri", "hmc", "-in", s, "-out", o + "_hmc"), omp))  # fmt: skip
    if nc:
        mat = d / "identity.mat"
        if make:
            mat.write_text("1 0 0 0\n0 1 0 0\n0 0 1 0\n0 0 0 1\n", encoding="utf-8")
        fsl = {
            "mcflirt": ["-in", s, "-out", o + "_mcf"],
            "bet2": [t1, o + "_bet2", "-m", "-e"],
            "bet": [t1, o + "_bet", "-m"],
            "betsurf": ["--t1only", "-m", t1, o + "_bet2_mesh.vtk", str(mat), o + "_bs"],
        }
        for name, a in fsl.items():
            out.append(Program(f"fsl {name} (replica, larmorx-nc)", (nc, "fsl", name, *a), omp))
    return out


__all__ = ["INPUTS", "Program", "descendants", "inputs", "probe", "programs", "threads"]
