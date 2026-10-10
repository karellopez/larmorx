# SPDX-License-Identifier: Apache-2.0
"""Benchmarks of the one-shot BOLD resampler: larmorx against fMRIPrep's implementation.

Both tools resample real BOLD runs from the test-data catalog, with seeded head motion (and a
seeded field map in Hz on the target grid where marked), onto fMRIPrep's targets: the
native boldref grid, the T1w grid through fMRIPrep's boldref-to-T1w affine, and
MNI152NLin2009cAsym res-2 through fMRIPrep's T1w-to-MNI ``.h5`` warp.

What is timed is ``ResampleSeries._run_interface`` without the file I/O: for fMRIPrep,
``load_transforms`` + ``ensure_positive_cosines`` + ``resample_image`` (``nthreads`` = the
thread count; volumes run concurrently on a thread pool); for larmorx,
``lx.transforms.resample_series`` (``n_threads``). Both start from the same in-memory source
and field map and read the same transform files; neither writes its output. Every larmorx
output is compared with fMRIPrep's.
"""

from __future__ import annotations

import argparse
import datetime as dt
import os
import sys
import tempfile
import warnings
from dataclasses import dataclass
from pathlib import Path

import larmorx_testdata as td
import numpy as np

from larmorx_validation import environment
from larmorx_validation.bench.harness import Measurement, measure
from larmorx_validation.parity.resample_series import _fieldmap_file, _motion_file
from larmorx_validation.report import environment_section, table, write_json

F = "openneuro-derivatives/ds000005-fmriprep/sub-01"
FUNC = f"{F}/func/sub-01_task-mixedgamblestask_run-1"
DS5 = "openneuro/ds000005/sub-01/func/sub-01_task-mixedgamblestask_run-01_bold.nii.gz"


@dataclass(frozen=True)
class Job:
    label: str
    source: str
    target: str | None  # None: the source's own grid
    transforms: tuple[str, ...]  # after the head-motion file
    fieldmap: bool
    pe_dir: str = "j"
    ro_time: float = 0.0312
    quick: bool = True
    heavy: bool = False  # fMRIPrep timed once (no warm-up)


JOBS = (
    Job(
        "ds000005 run 1 → boldref (native): 64×64×34, 240 volumes; motion + field map",
        DS5,
        f"{FUNC}_boldref.nii.gz",
        (),
        True,
    ),
    Job(
        "ds000005 run 1 → T1w grid: motion + boldref→T1w affine + field map",
        DS5,
        f"{FUNC}_space-T1w_boldref.nii.gz",
        (f"{FUNC}_from-scanner_to-T1w_mode-image_xfm.txt",),
        True,
    ),
    Job(
        "ds000005 run 1 → MNI152NLin2009cAsym res-2 (97×115×97): motion + affine + .h5 warp",
        DS5,
        f"{FUNC}_space-MNI152NLin2009cAsym_res-2_boldref.nii.gz",
        (
            f"{FUNC}_from-scanner_to-T1w_mode-image_xfm.txt",
            f"{F}/anat/sub-01_from-T1w_to-MNI152NLin2009cAsym_mode-image_xfm.h5",
        ),
        False,
        heavy=True,
    ),
    Job(
        "ds006010 sub-206 → native: 96×96×69, 113 volumes (uint16); motion + field map",
        "openneuro/ds006010/sub-206/func/sub-206_task-category_run-01_bold.nii.gz",
        None,
        (),
        True,
    ),
    Job(
        "ds005454 sub-16 rest → native: 160×160×96, 280 volumes (multiband 4); motion + field map",
        "openneuro/ds005454/sub-16/func/sub-16_task-rest_bold.nii.gz",
        None,
        (),
        True,
        quick=False,
        heavy=True,
    ),
)


def _fmt(s: float) -> str:
    return f"{s * 1000:.0f} ms" if s < 1 else f"{s:.2f} s"


def run_jobs(threads: list[int], repeats: int, quick: bool) -> list[Measurement]:
    import nibabel as nib
    from fmriprep.interfaces.resampling import resample_image
    from fmriprep.utils.transforms import load_transforms
    from sdcflows.utils.tools import ensure_positive_cosines

    import larmorx as lx

    results: list[Measurement] = []
    with tempfile.TemporaryDirectory(prefix="lx-bench-resample-") as tmp_name:
        tmp = Path(tmp_name)
        for job in (j for j in JOBS if j.quick or not quick):
            print(job.label, file=sys.stderr)
            src_path = td.get(job.source)
            src_nib = nib.load(src_path)
            data = np.asanyarray(src_nib.dataobj)  # in memory, stored type
            src_mem = nib.Nifti1Image(data, src_nib.affine, src_nib.header)
            src_lx = lx.Image(data, src_nib.affine, lx.load(src_path, mmap=True).header)
            if job.target is None:
                ref = tmp / "ref.nii.gz"
                nib.save(nib.Nifti1Image(np.zeros(data.shape[:3], np.float32), src_nib.affine), ref)
            else:
                ref = td.get(job.target)
            ref_img = nib.load(ref)
            hmc = _motion_file(src_path, 7, tmp / "hmc.txt")
            paths = [str(hmc)] + [str(td.get(p)) for p in job.transforms]
            fmap_img = fmap = None
            if job.fieldmap:
                fpath = _fieldmap_file(Path(ref), 3, tmp / "fmap.nii.gz")
                fmap_img = nib.load(fpath)
                fmap = fmap_img.get_fdata(dtype="f4")
                fmap_img = nib.Nifti1Image(fmap, fmap_img.affine)
            nvox = int(np.prod(ref_img.shape[:3])) * data.shape[3]
            nbytes = nvox * 4

            def run_fmriprep(
                t: int, job=job, src_mem=src_mem, ref_img=ref_img, paths=paths, fmap_img=fmap_img
            ):
                with warnings.catch_warnings():
                    warnings.simplefilter("ignore")
                    xfms = load_transforms(list(paths), [False])
                    # As ResampleSeries: reorient only for distortion correction.
                    source, pe_info = src_mem, None
                    if fmap_img is not None:
                        source, axcodes = ensure_positive_cosines(src_mem)
                        pe_axis = "ijk".index(job.pe_dir[0])
                        flip = (axcodes[pe_axis] in "LPI") ^ job.pe_dir.endswith("-")
                        ro = -job.ro_time if flip else job.ro_time
                        pe_info = [(pe_axis, ro)] * source.shape[3]
                    return resample_image(
                        source,
                        ref_img,
                        xfms,
                        fmap_img,
                        pe_info,
                        jacobian=True,
                        nthreads=t,
                        output_dtype="f4",
                        order=3,
                        mode="grid-constant",
                        cval=0.0,
                        prefilter=True,
                    )

            def run_lx(t: int, job=job, src_lx=src_lx, ref=ref, paths=paths, fmap=fmap):
                with warnings.catch_warnings():
                    warnings.simplefilter("ignore")
                    return lx.transforms.resample_series(
                        src_lx,
                        ref,
                        paths,
                        fieldmap=fmap,
                        pe_dir=job.pe_dir if fmap is not None else None,
                        ro_time=job.ro_time if fmap is not None else None,
                        jacobian=True,
                        n_threads=t,
                    )

            reference = None
            fmriprep_threads = [max(threads)] if job.heavy and quick else threads
            for t in fmriprep_threads:
                out: list = []
                seconds = measure(
                    lambda t=t, out=out: out.append(run_fmriprep(t)),
                    repeats=1 if job.heavy else repeats,
                    warmup=0 if job.heavy else 1,
                )
                if reference is None:
                    reference = np.asanyarray(out[-1].dataobj)
                results.append(
                    Measurement(
                        job.label, f"{t} threads", "fMRIPrep", seconds, nbytes, {"threads": t}
                    )
                )
                del out
            for t in threads:
                out = []
                seconds = measure(lambda t=t, out=out: out.append(run_lx(t)), repeats=repeats)
                got = np.asarray(out[-1].data)
                differing = int(np.count_nonzero(got.view(np.uint32) != reference.view(np.uint32)))
                results.append(
                    Measurement(
                        job.label,
                        f"{t} threads",
                        "larmorx",
                        seconds,
                        nbytes,
                        {"threads": t, "differing": differing, "values": int(got.size)},
                    )
                )
                del out
            del reference, data, src_mem, src_lx
    return results


def render(
    results: list[Measurement], env: dict, repeats: int, threads: list[int], command: str, load: str
) -> str:
    lines = [
        "# Benchmarks: one-shot BOLD resampling",
        "",
        "larmorx's `lx.transforms.resample_series` against fMRIPrep's one-shot resampler "
        "(`fmriprep/interfaces/resampling.py`, the code `ResampleSeries` runs) on real BOLD runs, "
        f"on {', '.join(map(str, threads))} threads. Median of {repeats} runs after a warm-up "
        "(fMRIPrep's slowest jobs once, without a warm-up).",
        "",
        f"- **Generated:** {dt.date.today().isoformat()} on {env['platform']}, with `{command}`; "
        f"load average before the run: {load}.",
        "- **What is timed:** the resampling as `ResampleSeries._run_interface` does it, without "
        "the file I/O: loading the transform files, reorienting the source (distortion "
        "correction), mapping the target grid, and resampling every volume (head motion, "
        "voxel-shift map, cubic B-spline interpolation in `grid-constant` mode, Jacobian). Both "
        "start from the same in-memory run and field map.",
        "- **Inputs:** seeded head motion (a random walk of about 1° and 1 mm) and, where "
        "marked, a seeded smooth field map (±120 Hz) on the target grid, PE `j`, readout 31.2 ms; "
        "fMRIPrep 21.0.1's own boldref→T1w affine and T1w→MNI warp for ds000005.",
        "- **Same result:** every larmorx output is compared with fMRIPrep's, value by value.",
        "",
    ]
    for job in dict.fromkeys(m.input for m in results):
        ms = [m for m in results if m.input == job]
        fm = {m.extra["threads"]: m for m in ms if m.tool == "fMRIPrep"}
        base = fm.get(1) or fm[min(fm)]
        rows = []
        for t in threads:
            if t in fm:
                m = fm[t]
                rows.append(
                    ("fMRIPrep", t, _fmt(m.median), "1.0×", f"{base.median / m.median:.1f}×", "")
                )
            for m in (m for m in ms if m.tool == "larmorx" and m.extra["threads"] == t):
                same = (
                    "bit-identical"
                    if m.extra["differing"] == 0
                    else f"{m.extra['differing']} of {m.extra['values']} values differ"
                )
                vs = f"**{fm[t].median / m.median:.1f}×**" if t in fm else "–"
                rows.append(
                    ("larmorx", t, _fmt(m.median), vs, f"{base.median / m.median:.1f}×", same)
                )
        lines += [
            f"## {job}",
            "",
            table(
                (
                    "Tool",
                    "Threads",
                    "Median",
                    "Speed-up vs fMRIPrep (same threads)",
                    f"vs fMRIPrep on {base.extra['threads']} thread{'s' if base.extra['threads'] != 1 else ''}",
                    "Output vs fMRIPrep",
                ),
                rows,
            ),
            "",
        ]
    lines += [
        "## Notes",
        "",
        "- **Where larmorx saves time.** fMRIPrep maps the target grid once per run, but then, "
        "per volume, copies the coordinates, applies the head-motion affine to all of them, adds "
        "the voxel-shift map, prefilters the volume (`map_coordinates` builds a float64 spline "
        "filter each call) and recomputes the Jacobian; each step allocates full-size arrays. "
        "larmorx computes the voxel-shift map and Jacobian once per run and moves each voxel's "
        "coordinates in registers inside the interpolation loop; the prefilter runs once per "
        "volume, line by line over contiguous rows.",
        "- **Exactness costs speed.** Each output value keeps SciPy's 64-tap summation order and "
        "numpy's fused multiply-adds for the head-motion product (a call into the C library's "
        "`fma` on x86-64 builds without FMA instructions). A separable evaluation would be several "
        "times faster but changes the last bits.",
        "- **Threads.** fMRIPrep resamples up to `nthreads` volumes at once (SciPy releases the "
        "GIL). larmorx gives whole volumes to threads when there are more volumes than threads, "
        "so at most `n_threads` prefiltered volumes are in memory, and otherwise splits each "
        "volume's voxels. The machine has 6 cores and 12 hardware threads.",
        "",
        "## Environment",
        "",
        environment_section(env),
        "",
    ]
    return "\n".join(lines)


def main(args: argparse.Namespace) -> int:
    repeats = 2 if args.quick else args.repeats
    threads = [t if t > 0 else os.cpu_count() or 1 for t in args.threads]
    load = " ".join(f"{v:.2f}" for v in os.getloadavg()) if hasattr(os, "getloadavg") else "n/a"
    results = run_jobs(threads, repeats, args.quick)
    env_info = environment.describe(("fmriprep", "nitransforms", "scipy", "nibabel"))
    command = "python -m larmorx_validation bench resample-series" + (
        " --quick"
        if args.quick
        else f" --repeats {repeats} --threads {' '.join(map(str, args.threads))}"
    )
    report = render(results, env_info, repeats, threads, command, load)
    if args.out:
        out = Path(args.out)
        out.mkdir(parents=True, exist_ok=True)
        (out / "resample-series.md").write_text(report, encoding="utf-8")
        write_json(
            out / "resample-series.json",
            {
                "environment": env_info,
                "repeats": repeats,
                "load_average": load,
                "results": [m.to_dict() for m in results],
            },
        )
        print(f"report: {out / 'resample-series.md'}", file=sys.stderr)
    else:
        print(report)
    return 0
