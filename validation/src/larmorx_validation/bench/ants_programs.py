# SPDX-License-Identifier: Apache-2.0
"""Benchmarks of ANTs' image programs (ImageMath, ThresholdImage, MultiplyImages, ...):
larmorx against ANTs 2.6.5's own binaries, on real images as fMRIPrep uses them.

Each :class:`Job` is one command line (the same placeholders as the parity harness,
:mod:`larmorx_validation.parity.ants_programs`). ANTs runs as its own process with
``ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS`` threads; larmorx runs the same command line
in-process through its console entry point, with the same variable. Both read the gzipped
inputs and write an **uncompressed** ``.nii`` (so that ITK's single-threaded gzip does not
dominate). Every larmorx output is compared with ANTs' (bit-identity is reported).

**Adding jobs** (another filter group): append :class:`Job` entries to :data:`JOBS`, or to a
list in your own module passed to :func:`run_jobs`.
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

from larmorx_validation import environment
from larmorx_validation.bench.harness import Measurement, measure
from larmorx_validation.parity import ants_programs as ap
from larmorx_validation.parity.ants_image_math.common import (
    BOLD_4D,
    GM,
    MNI_1MM,
    MNI_BRAIN_PROB,
    T1_MASK,
    T1_PREP,
    T1_RAW,
    WM,
)
from larmorx_validation.report import environment_section, table, write_json


@dataclass(frozen=True)
class Job:
    """One command line to time; ``{out}`` becomes an uncompressed ``.nii``."""

    label: str
    program: str
    args: tuple[str, ...]
    quick: bool = True


#: The benchmark jobs; add new ones here.
JOBS: list[Job] = [
    Job(
        "TruncateImageIntensity 0.01 0.999 256 (fMRIPrep's call), raw T1w ds000005 (176×256×256 int16)",
        "ImageMath",
        ("3", "{out}", "TruncateImageIntensity", T1_RAW, "0.01", "0.999", "256"),
    ),
    Job(
        "TruncateImageIntensity 0.01 0.999 256, MNI152NLin2009cAsym 1 mm (193×229×193)",
        "ImageMath",
        ("3", "{out}", "TruncateImageIntensity", MNI_1MM, "0.01", "0.999", "256"),
        quick=False,
    ),
    Job(
        "ThresholdImage 0.5 1, MNI brain probability map 1 mm",
        "ThresholdImage",
        ("3", MNI_BRAIN_PROB, "{out}", "0.5", "1"),
    ),
    Job(
        "ThresholdImage Otsu 3 with a brain mask, fMRIPrep T1w",
        "ThresholdImage",
        ("3", T1_PREP, "{out}", "Otsu", "3", T1_MASK),
        quick=False,
    ),
    Job(
        "MultiplyImages T1w × brain mask (fMRIPrep derivatives)",
        "MultiplyImages",
        ("3", T1_PREP, T1_MASK, "{out}"),
    ),
    Job(
        "ImageMath addtozero WM GM (probability maps)",
        "ImageMath",
        ("3", "{out}", "addtozero", WM, GM),
        quick=False,
    ),
    Job(
        "ImageMath Normalize, fMRIPrep T1w",
        "ImageMath",
        ("3", "{out}", "Normalize", T1_PREP),
        quick=False,
    ),
    Job(
        "ImageMath RescaleImage 0 1, MNI 1 mm",
        "ImageMath",
        ("3", "{out}", "RescaleImage", MNI_1MM, "0", "1"),
        quick=False,
    ),
    Job(
        "ImageMath 4 m 0.5, fMRIPrep BOLD series (4D, 28 MB gzipped)",
        "ImageMath",
        ("4", "{out}", "m", BOLD_4D, "0.5"),
        quick=False,
    ),
]


def _fmt(s: float) -> str:
    return f"{s * 1000:.0f} ms" if s < 1 else f"{s:.2f} s"


def _resolve(arg: str, tmp: Path, out: Path) -> str:
    if arg == "{out}":
        return str(out)
    return ap._resolve(arg, tmp, tmp)


def _compare(a_path: Path, b_path: Path) -> dict:
    import nibabel as nib
    import numpy as np

    a = np.asanyarray(nib.load(a_path).dataobj.get_unscaled())
    b = np.asanyarray(nib.load(b_path).dataobj.get_unscaled())
    if a.shape != b.shape or a.dtype != b.dtype:
        return {"bit_identical": False, "differing": -1}
    same = (a == b) | (np.isnan(a) & np.isnan(b)) if a.dtype.kind == "f" else a == b
    return {"bit_identical": bool(same.all()), "differing": int(a.size - same.sum())}


def run_jobs(
    jobs: list[Job], threads: list[int], ants_threads: list[int], repeats: int, quick: bool
) -> list[Measurement]:
    from larmorx.cli import run

    if ap.ants_bin() is None:
        raise SystemExit("the ANTs 2.6.5 oracle was not found (set LARMORX_ANTS_BIN)")
    results = []
    with tempfile.TemporaryDirectory(prefix="lx-bench-ants-") as tmp_name:
        tmp = Path(tmp_name)
        for job in (j for j in jobs if j.quick or not quick):
            print(job.label, file=sys.stderr)
            a_out, l_out = tmp / "ants.nii", tmp / "larmorx.nii"
            a_args = [_resolve(x, tmp, a_out) for x in job.args]
            l_args = [_resolve(x, tmp, l_out) for x in job.args]
            nbytes = 0
            for t in ants_threads:
                env = {**os.environ, "ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS": str(t)}

                def run_ants(env=env, a_out=a_out, a_args=a_args, job=job):
                    a_out.unlink(missing_ok=True)
                    p = subprocess.run(
                        [str(ap.ants_bin() / job.program), *a_args],
                        capture_output=True,
                        env=env,
                        check=False,
                    )
                    if p.returncode != 0 or not a_out.exists():
                        raise RuntimeError(f"ANTs failed: {p.stderr.decode()[-300:]}")

                seconds = measure(run_ants, repeats=repeats)
                nbytes = a_out.stat().st_size
                results.append(
                    Measurement(job.label, f"{t}", "ANTs 2.6.5", seconds, nbytes, {"threads": t})
                )
            for t in threads:

                def run_lx(t=t, l_out=l_out, l_args=l_args, job=job):
                    l_out.unlink(missing_ok=True)
                    os.environ["ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS"] = str(t)
                    with contextlib.redirect_stdout(io.StringIO()):
                        code = run(["larmorx", "ants", job.program, *l_args])
                    if code != 0:
                        raise RuntimeError(f"larmorx failed on {job.label}")

                seconds = measure(run_lx, repeats=repeats)
                extra = {"threads": t, **_compare(a_out, l_out)}
                results.append(Measurement(job.label, f"{t}", "larmorx", seconds, nbytes, extra))
            os.environ.pop("ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS", None)
    return results


def render(results: list[Measurement], env: dict, repeats: int, command: str) -> str:
    lines = [
        "# Benchmarks: ANTs image programs (ImageMath, ThresholdImage, MultiplyImages)",
        "",
        "larmorx against ANTs 2.6.5's own binaries on real images, as fMRIPrep uses these "
        f"programs. Median of {repeats} runs after a warm-up.",
        "",
        f"- **Generated:** {dt.date.today().isoformat()} on {env['platform']}, with `{command}`",
        "- **What is timed:** the whole command: reading the gzipped inputs, the operation, "
        "writing the output as uncompressed `.nii` (ITK compresses on one thread, which would "
        "dominate ANTs' time). ANTs runs as its own process; larmorx runs in-process through its "
        "console entry point. Both get the thread count through "
        "`ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS`.",
        "- **Same result:** every larmorx output is compared with ANTs' (all values).",
        "",
    ]
    for job in dict.fromkeys(m.input for m in results):
        ms = [m for m in results if m.input == job]
        base = next(m for m in ms if m.tool.startswith("ANTs") and m.extra["threads"] == 1)
        rows = []
        for m in ms:
            note = ""
            if m.tool == "larmorx":
                note = (
                    "bit-identical"
                    if m.extra.get("bit_identical")
                    else f"{m.extra.get('differing')} values differ"
                )
            rows.append(
                (
                    m.tool,
                    str(m.extra["threads"]),
                    _fmt(m.median),
                    f"**{base.median / m.median:.1f}×**" if m is not base else "1.0×",
                    note,
                )
            )
        lines += [
            f"## {job}",
            "",
            table(
                ("Tool", "Threads", "Median", "Speed-up vs ANTs, 1 thread", "Output vs ANTs"), rows
            ),
            "",
        ]
    lines += ["## Environment", "", environment_section(env), ""]
    return "\n".join(lines)


def main(args: argparse.Namespace) -> int:
    repeats = 2 if args.quick else args.repeats
    threads = [t if t > 0 else os.cpu_count() or 1 for t in args.threads]
    ants_threads = sorted({1, max(threads)})
    load_before = os.getloadavg() if hasattr(os, "getloadavg") else None
    results = run_jobs(JOBS, threads, ants_threads, repeats, args.quick)
    env_info = environment.describe(("nibabel",))
    env_info["load_average_before"] = load_before
    command = "python -m larmorx_validation bench ants-programs" + (
        " --quick"
        if args.quick
        else f" --repeats {repeats} --threads {' '.join(map(str, args.threads))}"
    )
    report = render(results, env_info, repeats, command)
    if args.out:
        out = Path(args.out)
        out.mkdir(parents=True, exist_ok=True)
        (out / "ants-programs.md").write_text(report, encoding="utf-8")
        write_json(
            out / "ants-programs.json",
            {
                "environment": env_info,
                "repeats": repeats,
                "results": [m.to_dict() for m in results],
            },
        )
        print(f"report: {out / 'ants-programs.md'}", file=sys.stderr)
    else:
        print(report)
    return 0
