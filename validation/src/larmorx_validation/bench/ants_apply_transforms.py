"""Benchmarks of antsApplyTransforms: larmorx against ANTs on real fMRIPrep resampling jobs.

Both tools run the same command line on the same files, in-process: ANTs through ANTsPy's
``ants.internal.get_lib_fn`` (the real antsApplyTransforms), larmorx through its console entry
point. Each timing covers reading the inputs and transforms, resampling, and writing the output.
Outputs are uncompressed ``.nii``, so gzip speed does not dominate. The Python API
(``lx.ants.apply_transforms``, no file output) is timed as well.

ITK reads its thread count (``ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS``) once per process, so
every thread count runs in its own worker process, where both tools use that many threads.
"""

from __future__ import annotations

import argparse
import contextlib
import datetime as dt
import io
import json
import os
import subprocess
import sys
import tempfile
from dataclasses import dataclass
from pathlib import Path

import larmorx_testdata as td

from larmorx_validation import environment
from larmorx_validation.bench.harness import Measurement, measure
from larmorx_validation.report import environment_section, table, write_json

F = "openneuro-derivatives/ds000005-fmriprep/sub-01"
FUNC = f"{F}/func/sub-01_task-mixedgamblestask_run-1"
T1W = f"{F}/anat/sub-01_desc-preproc_T1w.nii.gz"
MNI = f"{F}/anat/sub-01_space-MNI152NLin2009cAsym_res-2_desc-preproc_T1w.nii.gz"
TO_MNI = f"{F}/anat/sub-01_from-T1w_to-MNI152NLin2009cAsym_mode-image_xfm.h5"


@dataclass(frozen=True)
class Job:
    label: str
    image: str
    reference: str
    transform: str
    interpolation: str
    extra: tuple[str, ...] = ()
    quick: bool = True


JOBS = (
    Job(
        "boldref → T1w 1 mm, Linear",
        f"{FUNC}_boldref.nii.gz",
        T1W,
        f"{FUNC}_from-scanner_to-T1w_mode-image_xfm.txt",
        "Linear",
    ),
    Job(
        "boldref → T1w 1 mm, Lanczos",
        f"{FUNC}_boldref.nii.gz",
        T1W,
        f"{FUNC}_from-scanner_to-T1w_mode-image_xfm.txt",
        "LanczosWindowedSinc",
    ),
    Job("T1w → MNI 2 mm (.h5 affine + warp), Linear", T1W, MNI, TO_MNI, "Linear"),
    Job(
        "T1w → MNI 2 mm (.h5 affine + warp), Lanczos",
        T1W,
        MNI,
        TO_MNI,
        "LanczosWindowedSinc",
        quick=False,
    ),
    Job(
        "aseg → boldref, GenericLabel",
        f"{F}/anat/sub-01_desc-aseg_dseg.nii.gz",
        f"{FUNC}_boldref.nii.gz",
        f"{FUNC}_from-T1w_to-scanner_mode-image_xfm.txt",
        "GenericLabel",
        ("-u", "short"),
    ),
    Job(
        "BOLD 240 volumes → T1w grid, Lanczos (-e 3)",
        f"{FUNC}_desc-preproc_bold.nii.gz",
        f"{FUNC}_space-T1w_boldref.nii.gz",
        f"{FUNC}_from-scanner_to-T1w_mode-image_xfm.txt",
        "LanczosWindowedSinc",
        ("-e", "3"),
        quick=False,
    ),
)

PY_INTERP = {
    "Linear": "linear",
    "LanczosWindowedSinc": "lanczos",
    "GenericLabel": "genericlabel",
}


def _args(job: Job, out: Path) -> list[str]:
    return [
        "-d", "3",
        "-i", str(td.get(job.image)),
        "-r", str(td.get(job.reference)),
        "-t", str(td.get(job.transform)),
        "-n", job.interpolation,
        *job.extra,
        "-o", str(out),
    ]  # fmt: skip


def worker(threads: int, repeats: int, quick: bool) -> None:
    """Time every job with ``threads`` threads; print one JSON measurement per line."""
    import ants
    import nibabel as nib
    import numpy as np

    import larmorx as lx
    from larmorx.cli import run

    fn = ants.internal.get_lib_fn("antsApplyTransforms")
    jobs = [j for j in JOBS if j.quick or not quick]
    with tempfile.TemporaryDirectory(prefix="lx-bench-aat-") as tmp:
        tmp_dir = Path(tmp)
        for job in jobs:
            ants_out, lx_out = tmp_dir / "ants.nii", tmp_dir / "larmorx.nii"

            def run_ants(job=job, out=ants_out):
                with contextlib.redirect_stdout(io.StringIO()):
                    if fn(_args(job, out)) != 0:
                        raise RuntimeError(f"antsApplyTransforms failed on {job.label}")

            def run_lx(job=job, out=lx_out):
                argv = ["larmorx", "ants", "antsApplyTransforms", *_args(job, out)]
                if run(argv) != 0:
                    raise RuntimeError(f"larmorx failed on {job.label}")

            def run_py(job=job):
                time_series = "-e" in job.extra
                lx.ants.apply_transforms(
                    td.get(job.image),
                    td.get(job.reference),
                    [td.get(job.transform)],
                    interpolation=PY_INTERP[job.interpolation],
                    time_series=time_series,
                    n_threads=threads,
                )

            timings = {
                "ANTs 2.6.5": measure(run_ants, repeats=repeats),
                "larmorx (CLI)": measure(run_lx, repeats=repeats),
                "larmorx (Python, no file output)": measure(run_py, repeats=repeats),
            }
            a = np.asanyarray(nib.load(ants_out).dataobj)
            b = np.asanyarray(nib.load(lx_out).dataobj)
            same = bool(a.shape == b.shape and np.array_equal(a, b))
            rel = float(np.abs(a.astype(np.float64) - b).max() / (np.abs(a).max() or 1.0))
            for tool, seconds in timings.items():
                m = Measurement(
                    job.label,
                    f"{threads} thread{'s' if threads != 1 else ''}",
                    tool,
                    seconds,
                    int(a.size * a.dtype.itemsize),
                    {
                        "threads": threads,
                        "bit_identical": same,
                        "max_rel_diff": rel,
                        "shape": list(a.shape),
                    },
                )
                print(json.dumps(m.to_dict()), flush=True)


def _fmt(s: float) -> str:
    return f"{s * 1000:.0f} ms" if s < 1 else f"{s:.2f} s"


def render(
    results: list[Measurement], env: dict, repeats: int, threads: list[int], command: str
) -> str:
    jobs = list(dict.fromkeys(m.input for m in results))
    lines = [
        "# Benchmarks: antsApplyTransforms",
        "",
        f"larmorx against antsApplyTransforms (ANTs 2.6.5 on ITK 5.4.5, through ANTsPy) on real fMRIPrep "
        f"resampling jobs (ds000005, sub-01), with {', '.join(map(str, threads))} threads. Median of {repeats} runs after a warm-up.",
        "",
        f"- **Generated:** {dt.date.today().isoformat()} on {env['platform']}, with `{command}`",
        "- **What is timed:** the whole command: reading the image and the transforms, resampling, "
        "writing the output (uncompressed `.nii`, float64 unless `-u`). Both run in-process with the "
        "same arguments; ITK's thread count is set per worker process. The Python API column skips the "
        "file output.",
        "- **Same result:** every larmorx output below is compared with ANTs' (bit-identical, or the "
        "largest relative difference).",
        "",
    ]
    for job in jobs:
        ms = [m for m in results if m.input == job]
        rows = []
        for t in threads:
            by_tool = {m.tool: m for m in ms if m.extra["threads"] == t}
            if not by_tool:
                continue
            a = by_tool["ANTs 2.6.5"]
            c = by_tool["larmorx (CLI)"]
            p = by_tool["larmorx (Python, no file output)"]
            rows.append(
                (
                    str(t),
                    _fmt(a.median),
                    _fmt(c.median),
                    f"**{a.median / c.median:.1f}×**",
                    _fmt(p.median),
                    "bit-identical"
                    if c.extra["bit_identical"]
                    else f"≤ {c.extra['max_rel_diff']:.0e} rel.",
                )
            )
        shape = "×".join(map(str, ms[0].extra["shape"]))
        lines += [
            f"## {job}",
            "",
            f"Output {shape}.",
            "",
            table(
                ("Threads", "ANTs", "larmorx CLI", "Speed-up", "larmorx Python", "Output vs ANTs"),
                rows,
            ),
            "",
        ]
    lines += ["## Environment", "", environment_section(env), ""]
    return "\n".join(lines)


def main(args: argparse.Namespace) -> int:
    repeats = 2 if args.quick else args.repeats
    threads = [t if t > 0 else os.cpu_count() or 1 for t in args.threads]
    results: list[Measurement] = []
    for t in threads:
        print(f"worker: {t} threads", file=sys.stderr)
        env = dict(os.environ, ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS=str(t))
        cmd = [sys.executable, "-W", "ignore", "-m", "larmorx_validation.bench.ants_apply_transforms",
               "--threads", str(t), "--repeats", str(repeats)] + (["--quick"] if args.quick else [])  # fmt: skip
        proc = subprocess.run(cmd, env=env, capture_output=True, text=True, check=False)
        if proc.returncode != 0:
            print(proc.stderr, file=sys.stderr)
            return 1
        for line in proc.stdout.splitlines():
            d = json.loads(line)
            results.append(
                Measurement(
                    d["input"], d["operation"], d["tool"], d["seconds"], d["data_bytes"], d["extra"]
                )
            )
    env_info = environment.describe(("antspyx", "nibabel", "h5py"))
    command = "python -m larmorx_validation bench ants-apply-transforms" + (
        " --quick"
        if args.quick
        else f" --repeats {repeats} --threads {' '.join(map(str, args.threads))}"
    )
    report = render(results, env_info, repeats, threads, command)
    if args.out:
        out = Path(args.out)
        out.mkdir(parents=True, exist_ok=True)
        (out / "ants-apply-transforms.md").write_text(report, encoding="utf-8")
        write_json(
            out / "ants-apply-transforms.json",
            {
                "environment": env_info,
                "repeats": repeats,
                "results": [m.to_dict() for m in results],
            },
        )
        print(f"report: {out / 'ants-apply-transforms.md'}", file=sys.stderr)
    else:
        print(report)
    return 0


if __name__ == "__main__":
    p = argparse.ArgumentParser()
    p.add_argument("--threads", type=int, required=True)
    p.add_argument("--repeats", type=int, default=3)
    p.add_argument("--quick", action="store_true")
    a = p.parse_args()
    worker(a.threads, a.repeats, a.quick)
