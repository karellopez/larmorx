# SPDX-License-Identifier: Apache-2.0
"""Speed and peak memory of 3dvolreg on fMRIPrep's call: AFNI 26.3.05, the replica and the
clean-room original (black boxes), at one thread and many.

Written by the clean-room implementer of ``specs/3dvolreg.md``. AFNI runs through the
oracle-output cache with ``peak=True`` (its peak resident memory measured by ``/usr/bin/time``
and kept in the entry; served afterwards: the peak is the entry's, the seconds are not kept, so
a served AFNI line shows 0 s and AFNI's time is its first run's); larmorx runs from the
command line under ``/usr/bin/time -v``. The inputs are fMRIPrep's own ``_sliced`` files
(``oracles/inputs/afni-volreg/fmriprep/``, made by the replica part's suite).

    python -P perf.py [--threads 1,14] [--impl original,replica] [--no-afni] label ...

Prints one line per run: label, program, threads, seconds, peak MiB, and the allowance of the
memory rule (AFNI's peak times 1.1 plus 8 MiB).
"""

from __future__ import annotations

import argparse
import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
WORKSPACE = Path(os.environ.get("LARMORX_WORKSPACE", REPO.parent))
AFNI = WORKSPACE / "oracles/afni-26.3.05/bin/3dvolreg"
LX = os.environ.get("LX_BIN", str(REPO / "target/release/larmorx"))
FMRIPREP_INPUTS = WORKSPACE / "oracles/inputs/afni-volreg/fmriprep"
sys.path.insert(0, str(REPO / "validation/src"))

CALL = [
    "-Fourier",
    "-twopass",
    "-zpad",
    "4",
    "-prefix",
    "o.nii.gz",
    "-1Dfile",
    "o.1D",
    "-1Dmatrix_save",
    "o.aff12.1D",
    "-maxdisp1D",
    "o_md.1D",
]


def env_for(work: Path, threads: int) -> dict[str, str]:
    home = work / "home"
    home.mkdir(parents=True, exist_ok=True)
    return {
        "HOME": str(home),
        "PATH": "/usr/bin:/bin",
        "AFNI_DONT_LOGFILE": "YES",
        "OMP_NUM_THREADS": str(threads),
    }


def afni(label: str, work: Path) -> tuple[float, float, bool]:
    from larmorx_validation import oracle_cache

    work.mkdir(parents=True, exist_ok=True)
    r = oracle_cache.run(
        [str(AFNI), *CALL, str(FMRIPREP_INPUTS / f"{label}_sliced.nii.gz")],
        env=env_for(work, 1),
        cwd=work,
        outputs=[work],
        policy=oracle_cache.Policy(
            threads=1,
            family="afni",
            nifti_extensions="AFNI's history: the date and the output's path",
        ),
        runner="afni-volreg-original-perf",
        peak=True,
    )
    if r.returncode != 0:
        raise SystemExit(f"AFNI failed on {label}: {r.stderr[-500:]}")
    return r.seconds, r.peak_mib or 0.0, r.cached


def larmorx(label: str, impl: str, threads: int, work: Path) -> tuple[float, float]:
    work.mkdir(parents=True, exist_ok=True)
    env = {**env_for(work, threads), "LARMORX_IMPLEMENTATION": impl}
    report = work / "time.txt"
    cmd = [
        "/usr/bin/time",
        "-v",
        "-o",
        str(report),
        LX,
        "afni",
        "3dvolreg",
        *CALL,
        str(FMRIPREP_INPUTS / f"{label}_sliced.nii.gz"),
    ]
    r = subprocess.run(cmd, cwd=work, env=env, capture_output=True)
    if r.returncode != 0:
        raise SystemExit(f"{impl} failed on {label}: {r.stderr[-500:]!r}")
    text = report.read_text()
    peak = int(re.search(r"Maximum resident set size \(kbytes\): (\d+)", text).group(1)) / 1024
    wall = re.search(r"Elapsed \(wall clock\) time \(h:mm:ss or m:ss\): (\S+)", text).group(1)
    parts = [float(x) for x in wall.split(":")]
    seconds = parts[-1] + 60 * parts[-2] + (3600 * parts[-3] if len(parts) > 2 else 0)
    return seconds, peak


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("labels", nargs="+")
    ap.add_argument("--threads", default="1,14")
    ap.add_argument("--impl", default="original,replica")
    ap.add_argument("--no-afni", action="store_true")
    a = ap.parse_args(argv)
    threads = [int(t) for t in a.threads.split(",")]
    with tempfile.TemporaryDirectory(
        prefix="volreg-perf-", dir=os.environ.get("VOLREG_WORK")
    ) as tmp:
        root = Path(tmp)
        for label in a.labels:
            allowance = None
            if not a.no_afni:
                s, mib, cached = afni(label, root / f"{label}-afni")
                allowance = 1.1 * mib + 8
                print(
                    f"{label:18} afni      t=1   {s:8.2f} s {mib:8.1f} MiB"
                    f"{'  (served: the time of its first run is not kept)' if cached else ''}",
                    flush=True,
                )
            for impl in [i for i in a.impl.split(",") if i]:
                for t in threads:
                    s, mib = larmorx(label, impl, t, root / f"{label}-{impl}-{t}")
                    note = f"  allowance {allowance:.1f}" if allowance and t == 1 else ""
                    print(
                        f"{label:18} {impl:9} t={t:<3} {s:8.2f} s {mib:8.1f} MiB{note}", flush=True
                    )
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
