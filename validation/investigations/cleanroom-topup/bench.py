# SPDX-License-Identifier: Apache-2.0
"""topup's benchmark (``docs/benchmarks/fsl-topup.md``): fMRIPrep's topup call (sdcflows'
``b02b0.cnf``) on its two oracle runs (ds005040, ds001600) and one applytopup least-squares
restoration (``--method=lsr``, the default) of ds005040's pair with FSL's topup result, with
FSL 6.0.7.23 (on one thread, its ``--nthr`` default, and with ``--nthr=<all>``), the replica
(``larmorx-nc``, one thread: FSL's one-thread result) and the clean-room original on each of
``--threads``. Wall time of the whole command, one run each (informal).

FSL's runs here are timed, so the oracle cache cannot serve them: each goes through
``oracle_cache.run`` with a ``never`` reason (run fresh, not stored), and the script counts them.
The replica (``LARMORX_NC_BIN``, a ``larmorx-nc`` program) and the original (``LARMORX_TOPUP_BIN``,
a ``larmorx`` binary) run as their suites run them, in FSL's clean environment. Each run's
outputs are checked: the replica's against FSL's one-thread run byte for byte (decompressed), the
original's the same files at every thread count, byte for byte.

Run it inside a memory box::

    LARMORX_TOPUP_BIN=target/release/larmorx LARMORX_NC_BIN=<scratch>/nc-target/release/larmorx-nc \\
        scripts/orchestration/boxed.sh 16 python -P validation/investigations/cleanroom-topup/bench.py \\
        --threads 1 14 --fsl-threads 14 --json docs/benchmarks/fsl-topup.json
"""

from __future__ import annotations

import argparse
import gzip
import json
import os
import platform
import shutil
import subprocess
import tempfile
import time
from pathlib import Path

from larmorx_validation.parity import defects
from larmorx_validation.parity import fsl_topup as orig
from larmorx_validation.parity import fsl_topup_replica as rep

RUNS = ("ds005040-sub-002-noreconall", "ds001600-sub-1-noreconall")
NEVER = "a timed benchmark run: its duration is the measurement (docs/benchmarks/fsl-topup.md)"


def outputs(d: Path, before: set[str]) -> dict[str, bytes]:
    out = {}
    for p in sorted(d.iterdir()):
        if p.name in before or not p.is_file():
            continue
        b = p.read_bytes()
        out[p.name] = gzip.decompress(b) if p.name.endswith(".gz") else b
    return out


def timed(f):
    t0 = time.perf_counter()
    r = f()
    return r, round(time.perf_counter() - t0, 2)


def fsl(args: list[str], d: Path, threads: int, program: str = "topup"):
    p = rep.oracle_run(args, cwd=d, settings={}, never=NEVER, threads=threads, program=program)
    return rep.Completed(p.returncode, p.stdout, p.stderr)


def compare_programs(
    name: str,
    program: str,
    args: list[str],
    prepare,
    threads: list[int],
    fsl_threads: int,
    count: list[int],
    keep: Path | None = None,
) -> dict:
    """FSL (one thread, and ``fsl_threads`` for topup), the replica and the original on
    ``threads``, each in its own directory made by ``prepare``; FSL's one-thread outputs copied
    into ``keep``."""
    row: dict = {"call": name, "program": program}
    with tempfile.TemporaryDirectory(prefix="lx-topup-bench-") as t_:
        t = Path(t_)
        fsl_runs = [1] + ([fsl_threads] if program == "topup" and fsl_threads > 1 else [])
        keys = [f"fsl{n}" for n in fsl_runs] + ["replica"] + [f"original{n}" for n in threads]
        dirs = {k: t / k for k in keys}
        before = {}
        for k, d in dirs.items():
            d.mkdir()
            prepare(d)
            before[k] = {p.name for p in d.iterdir()}
        row["fsl_seconds"] = {}
        for n in fsl_runs:
            a = args if n == 1 else [*args, f"--nthr={n}"]
            r, s = timed(lambda a=a, n=n: fsl(a, dirs[f"fsl{n}"], n, program))
            count[0] += 1
            assert r.code == 0, (name, n, r.stderr[-300:])
            row["fsl_seconds"][str(n)] = s
        ref = outputs(dirs["fsl1"], before["fsl1"])
        r, s = timed(
            lambda: rep.run_replica(
                program, args, cwd=dirs["replica"], settings={}, mode=defects.REPRODUCE
            )
        )
        row["replica_seconds"] = s
        row["replica_bit_identical"] = (
            r.code == 0 and outputs(dirs["replica"], before["replica"]) == ref
        )
        row["original_seconds"] = {}
        first = None
        for n in threads:
            d = dirs[f"original{n}"]
            r, s = timed(
                lambda d=d, n=n: orig.run_original(
                    args, cwd=d, settings={}, mode=defects.FIX, threads=n, program=program
                )
            )
            assert r.code == 0, (name, n, r.stderr[-300:])
            row["original_seconds"][str(n)] = s
            got = outputs(d, before[f"original{n}"])
            assert set(got) == set(ref), (name, n, sorted(set(got) ^ set(ref)))
            first = first or got
            row.setdefault("original_same_bits_at_every_thread_count", True)
            row["original_same_bits_at_every_thread_count"] &= got == first
        if keep is not None:
            for f in ref:
                shutil.copy(dirs["fsl1"] / f, keep / f)
    return row


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--threads", type=int, nargs="+", default=[1, 14])
    ap.add_argument("--fsl-threads", type=int, default=14)
    ap.add_argument("--json", type=Path)
    a = ap.parse_args()
    calls = dict(rep.fmriprep_calls())
    load = os.getloadavg()[0]
    count = [0]
    rows = []
    keep = Path(tempfile.mkdtemp(prefix="lx-topup-bench-result-"))
    for run in RUNS:
        node = calls[run]
        args = rep.fmriprep_args(node)

        def prepare(d: Path, node=node) -> None:
            assert rep._prepare_fmriprep(node, d)

        first = run == RUNS[0]
        row = compare_programs(
            f"fmriprep/{run}",
            "topup",
            args,
            prepare,
            a.threads,
            a.fsl_threads,
            count,
            keep if first else None,
        )
        if first:
            # the inputs and FSL's one-thread topup result of this call, for applytopup below
            names = {k: v for k, _, v in (x.partition("=") for x in args)}
            prepare(keep)
        rows.append(row)
        print(json.dumps(row))
    # applytopup's least-squares restoration of ds005040's pair with FSL's topup result
    import nibabel as nib
    import numpy as np

    img = nib.load(str(keep / names["--imain"]))
    data = np.asanyarray(img.dataobj)
    base = names["--out"]

    def prepare_lsr(d: Path) -> None:
        for k, n in enumerate(("up", "dn")):
            nib.save(nib.Nifti1Image(data[..., k], img.affine, img.header), str(d / f"{n}.nii.gz"))
        shutil.copy(keep / names["--datain"], d / "enc.txt")
        shutil.copy(keep / f"{base}_fieldcoef.nii.gz", d / "tu_fieldcoef.nii.gz")
        shutil.copy(keep / f"{base}_movpar.txt", d / "tu_movpar.txt")

    lsr_args = [
        "--imain=up.nii.gz,dn.nii.gz",
        "--inindex=1,2",
        "--datain=enc.txt",
        "--topup=tu",
        "--out=hifi",
    ]
    row = compare_programs(
        "applytopup/lsr ds005040", "applytopup", lsr_args, prepare_lsr, a.threads, 1, count
    )
    rows.append(row)
    print(json.dumps(row))
    shutil.rmtree(keep, ignore_errors=True)
    print(f"FSL runs (fresh, timed, not stored): {count[0]}")
    if a.json:
        rev = subprocess.run(
            ["git", "rev-parse", "--short", "HEAD"], capture_output=True, text=True, check=False
        ).stdout.strip()
        a.json.write_text(
            json.dumps(
                {
                    "tool": "fsl topup and applytopup (clean-room original)",
                    "commit": rev,
                    "machine": platform.machine(),
                    "load_average_before": round(load, 1),
                    "threads": a.threads,
                    "fsl_threads": a.fsl_threads,
                    "fsl_runs_timed": count[0],
                    "environment": "FSL's clean environment (FSL off the PATH, "
                    "FSLOUTPUTTYPE=NIFTI_GZ, OPENBLAS_CORETYPE=Haswell); OMP_NUM_THREADS for the "
                    "original, --nthr for FSL",
                    "rows": rows,
                },
                indent=1,
            )
            + "\n",
            encoding="utf-8",
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
