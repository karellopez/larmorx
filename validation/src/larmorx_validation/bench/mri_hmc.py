# SPDX-License-Identifier: Apache-2.0
"""Benchmarks of head-motion correction: ``larmorx mri hmc`` against FSL 6.0.7's ``mcflirt``.

Both run fMRIPrep's command (``-reffile <HMC reference> -mats``, ``FSLOUTPUTTYPE=NIFTI_GZ``) on
real BOLD runs from the test-data catalog; the references are the ones the recorded oracle runs
used (fMRIPrep's published boldref for ds000005, a median of the first ten volumes otherwise).
mcflirt runs as its own process (it is single-threaded; set ``LARMORX_MCFLIRT`` to its path,
default ``~/fsl/bin/mcflirt``); larmorx runs in-process through its console entry point with
``OMP_NUM_THREADS`` threads, and through the Python API without resampling (what fMRIPrep
needs: the matrices only). Each timing covers the whole command: reading, estimation, the
corrected series (gzipped) and the matrices.
"""

from __future__ import annotations

import argparse
import contextlib
import datetime as dt
import io
import os
import subprocess
import sys
import tempfile
from dataclasses import dataclass
from pathlib import Path

import numpy as np

from larmorx_validation import environment
from larmorx_validation.bench.harness import Measurement, measure
from larmorx_validation.parity.mri_hmc import (
    _fov_centre,
    _read_mats,
    _resolve,
    oracle_dir,
    rms_deviation,
)
from larmorx_validation.report import environment_section, table, write_json


@dataclass(frozen=True)
class Job:
    label: str
    oracle_run: str
    quick: bool = True


JOBS = (
    Job("ds000005 sub-01 run 1: 64×64×34, 240 volumes, int16", "real/ds000005/fmriprep"),
    Job(
        "ds006010 sub-206: 96×96×69, 113 volumes, uint16 (multiband)",
        "real/ds006010/fmriprep",
        quick=False,
    ),
    Job(
        "ds000258 sub-21262 echo 4: 64×64×30, 239 volumes, int16 (big-endian)",
        "real/ds000258-bigendian/fmriprep",
        quick=False,
    ),
)


def mcflirt_binary() -> str | None:
    path = Path(os.environ.get("LARMORX_MCFLIRT", str(Path.home() / "fsl" / "bin" / "mcflirt")))
    return str(path) if path.is_file() else None


def _fmt(s: float) -> str:
    return f"{s * 1000:.0f} ms" if s < 1 else f"{s:.2f} s"


def run_jobs(
    threads: list[int], repeats: int, mcflirt_repeats: int, quick: bool
) -> list[Measurement]:
    import json

    import larmorx as lx
    from larmorx.cli import run

    binary = mcflirt_binary()
    root = oracle_dir()
    if binary is None or root is None:
        raise SystemExit("mcflirt (LARMORX_MCFLIRT) or the recorded runs were not found")
    fsl_env = {
        "HOME": os.environ.get("HOME", ""),
        "PATH": "/usr/bin:/bin",
        "FSLDIR": str(Path(binary).resolve().parents[1]),
        "FSLOUTPUTTYPE": "NIFTI_GZ",
    }
    results = []
    with tempfile.TemporaryDirectory(prefix="lx-bench-hmc-") as tmp_name:
        tmp = Path(tmp_name)
        for job in (j for j in JOBS if j.quick or not quick):
            print(job.label, file=sys.stderr)
            rec = json.loads(
                (root / "runs" / job.oracle_run / "run.json").read_text(encoding="utf-8")
            )
            src = Path(_resolve(rec["inputs"]["in"]["path"]))
            ref = Path(_resolve(rec["inputs"]["ref"]["path"]))
            import nibabel as nib

            shape = nib.load(src).shape
            nbytes = int(np.prod(shape)) * 4

            def run_fsl(src=src, ref=ref):
                out = tmp / "fsl" / "out_mcf.nii.gz"
                out.parent.mkdir(exist_ok=True)
                for p in out.parent.glob("*"):
                    if p.is_dir():
                        for q in p.iterdir():
                            q.unlink()
                        p.rmdir()
                    else:
                        p.unlink()
                args = ["-in", str(src), "-out", str(out), "-reffile", str(ref), "-mats"]
                p = subprocess.run([binary, *args], env=fsl_env, capture_output=True, check=False)
                if p.returncode != 0:
                    raise RuntimeError(f"mcflirt failed: {p.stderr[-300:]}")

            seconds = measure(run_fsl, repeats=mcflirt_repeats, warmup=0)
            fsl_mats, _ = _read_mats(tmp / "fsl" / "out_mcf.nii.gz.mat")
            centre = _fov_centre(ref)
            results.append(
                Measurement(
                    job.label, "1 thread", "mcflirt (FSL 6.0.7)", seconds, nbytes, {"threads": 1}
                )
            )
            for t in threads:
                out = tmp / f"lx{t}" / "out_mcf.nii.gz"

                def run_lx(src=src, ref=ref, t=t, out=out):
                    out.parent.mkdir(exist_ok=True)
                    mats = Path(f"{out}.mat")
                    if mats.exists():
                        for q in mats.iterdir():
                            q.unlink()
                        mats.rmdir()
                    os.environ["OMP_NUM_THREADS"] = str(t)
                    with contextlib.redirect_stderr(io.StringIO()):
                        code = run(
                            [
                                "larmorx",
                                "mri",
                                "hmc",
                                "-in",
                                str(src),
                                "-out",
                                str(out),
                                "-reffile",
                                str(ref),
                                "-mats",
                            ]
                        )
                    if code != 0:
                        raise RuntimeError("larmorx failed")

                def run_py(src=src, ref=ref, t=t):
                    lx.mri.hmc(src, reference=ref, resample=False, n_threads=t)

                cli = measure(run_lx, repeats=repeats)
                mats, _ = _read_mats(Path(f"{out}.mat"))
                devs = [rms_deviation(a, b, centre) for a, b in zip(fsl_mats, mats, strict=True)]
                extra = {
                    "threads": t,
                    "rms_median": float(np.median(devs)),
                    "rms_p95": float(np.percentile(devs, 95)),
                }
                label = f"{t} thread{'s' if t != 1 else ''}"
                results.append(Measurement(job.label, label, "larmorx (CLI)", cli, nbytes, extra))
                py = measure(run_py, repeats=repeats)
                results.append(
                    Measurement(
                        job.label, label, "larmorx (Python, matrices only)", py, nbytes, extra
                    )
                )
            os.environ.pop("OMP_NUM_THREADS", None)
    return results


def render(
    results: list[Measurement], env: dict, repeats: int, threads: list[int], command: str, load: str
) -> str:
    lines = [
        "# Benchmarks: head-motion correction (`lx.mri.hmc`)",
        "",
        "larmorx's clean-room head-motion correction against FSL 6.0.7's mcflirt (single-threaded) "
        f"on real BOLD runs, both with fMRIPrep's command, larmorx on {', '.join(map(str, threads))} "
        f"threads. larmorx: median of {repeats} runs after a warm-up.",
        "",
        f"- **Generated:** {dt.date.today().isoformat()} on {env['platform']}, with `{command}`; "
        f"load before the run: {load}.",
        "- **What is timed:** the whole command `-in <run> -out <out>.nii.gz -reffile <ref> -mats`: "
        "reading the gzipped run, the three-stage estimation, the corrected series (written "
        "gzipped) and the `.mat` files. mcflirt runs as its own process; larmorx in-process "
        "through its console entry point. The Python column (`lx.mri.hmc(..., resample=False)`) "
        "estimates only, which is all fMRIPrep uses.",
        "- **Same result:** each larmorx run's matrices are compared with mcflirt's (RMS "
        "deviation, radius 80 mm; the parity record has the full comparison).",
        "",
    ]
    for job in dict.fromkeys(m.input for m in results):
        ms = [m for m in results if m.input == job]
        fsl = next(m for m in ms if m.tool.startswith("mcflirt"))
        rows = [(fsl.tool, "1", _fmt(fsl.median), "1.0×", "")]
        for t in threads:
            for m in (m for m in ms if m.tool.startswith("larmorx") and m.extra["threads"] == t):
                note = (
                    f"RMS dev. median {m.extra['rms_median']:.4f} mm, 95th pct "
                    f"{m.extra['rms_p95']:.4f} mm"
                    if m.tool == "larmorx (CLI)"
                    else ""
                )
                rows.append(
                    (m.tool, str(t), _fmt(m.median), f"**{fsl.median / m.median:.1f}×**", note)
                )
        lines += [
            f"## {job}",
            "",
            table(
                ("Tool", "Threads", "Median", "Speed-up vs mcflirt", "Matrices vs mcflirt"), rows
            ),
            "",
        ]
    lines += [
        "## Notes",
        "",
        "- **Where the parallelism comes from.** The first stage (8 mm) is a chain: each volume "
        "starts from its predecessor's result, so its volumes run one after another and each cost "
        "evaluation samples its grid slices in parallel. The second and third stages (4 mm) start "
        "every volume from its own previous result, so the volumes run in parallel. Every "
        "evaluation gives the same bits on any number of threads.",
        "- **Single-thread speed** is close to mcflirt's: the cost function follows mcflirt's "
        "float32 arithmetic point by point (that is what keeps the results within mcflirt's own "
        "variability), so the gain comes from the threads.",
        "",
        "## Environment",
        "",
        environment_section(env),
        "",
    ]
    return "\n".join(lines)


def main(args: argparse.Namespace) -> int:
    repeats = 1 if args.quick else args.repeats
    threads = [t if t > 0 else os.cpu_count() or 1 for t in args.threads]
    load = " ".join(f"{v:.2f}" for v in os.getloadavg())
    results = run_jobs(threads, repeats, 1, args.quick)
    env_info = environment.describe(("nibabel",))
    command = "python -m larmorx_validation bench mri-hmc" + (
        " --quick"
        if args.quick
        else f" --repeats {repeats} --threads {' '.join(map(str, args.threads))}"
    )
    report = render(results, env_info, repeats, threads, command, f"load average {load}")
    if args.out:
        out = Path(args.out)
        out.mkdir(parents=True, exist_ok=True)
        (out / "mri-hmc.md").write_text(report, encoding="utf-8")
        write_json(
            out / "mri-hmc.json",
            {
                "environment": env_info,
                "repeats": repeats,
                "load": load,
                "results": [m.to_dict() for m in results],
            },
        )
        print(f"report: {out / 'mri-hmc.md'}", file=sys.stderr)
    else:
        print(report)
    return 0
