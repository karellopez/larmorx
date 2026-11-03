# SPDX-License-Identifier: Apache-2.0
"""FAST's benchmark (``docs/benchmarks/fsl-fast.md``): fMRIPrep's call (``-I 0 -N -p -g -S 1``) on
the standard tier's three oracle runs, with FSL 6.0.7.23, the replica (``larmorx-nc fsl fast``)
and the clean-room original on each of ``--threads``. Wall time of the whole command, one run
each (informal).

FSL is never run again: its time is that of the run the oracle cache stored for the
``fsl-fast-replica`` suite's key (the cache serves the run; ``Tally.saved_seconds`` is the stored
run's time), so it was measured when that entry was made, on this machine. The replica
(``LARMORX_NC_BIN``, else the workspace's ``crates-nc/target/release/larmorx-nc``) and the
original (``LARMORX_FAST_BIN``, a ``larmorx`` binary) run as their suites run them, in FSL's
clean environment, timed here. Each run's outputs are checked: the replica's against FSL's
byte for byte (decompressed), the original's files present.

Run it inside a memory box::

    LARMORX_FAST_BIN=target/release/larmorx scripts/orchestration/boxed.sh 8 \\
        python -P validation/investigations/cleanroom-fast/bench.py --threads 1 14 \\
        --json docs/benchmarks/fsl-fast.json
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

from larmorx_validation.oracle_cache import core
from larmorx_validation.parity import defects
from larmorx_validation.parity import fsl_fast as orig
from larmorx_validation.parity import fsl_fast_replica as rep

RUNS = (
    "ds000005-sub-01-noreconall",
    "ds000228-sub-pixar008-noreconall",
    "ds005003-sub-RS001-noreconall",
)


def images(d: Path) -> dict[str, bytes]:
    return {
        p.name: gzip.decompress(p.read_bytes())
        for p in sorted(d.iterdir())
        if p.name.endswith(".nii.gz") and "_" in p.name
    }


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--threads", type=int, nargs="+", default=[1, 14])
    ap.add_argument("--json", type=Path)
    a = ap.parse_args()
    calls = {run: node for run, node in rep.fmriprep_calls() if run in RUNS}
    load = os.getloadavg()[0]
    rows = []
    for run in RUNS:
        node = calls[run]
        src = rep.fmriprep_input(run, str(node))
        args = rep.fmriprep_args(node)
        with tempfile.TemporaryDirectory(prefix="lx-fast-bench-") as t:
            t = Path(t)
            dirs = {k: t / k for k in ["fsl", "replica", *(f"original{n}" for n in a.threads)]}
            for d in dirs.values():
                d.mkdir()
                shutil.copy(src, d / args[-1])
            core.begin_case(f"bench/{run}")
            f0 = time.perf_counter()
            fsl = rep.run_oracle(args, cwd=dirs["fsl"], settings={})
            wall = time.perf_counter() - f0
            tally = core.end_case()
            fsl_s = tally.saved_seconds if tally.served else wall
            r0 = time.perf_counter()
            r = rep.run_replica(
                rep.PROGRAM, args, cwd=dirs["replica"], settings={}, mode=defects.REPRODUCE
            )
            replica_s = time.perf_counter() - r0
            same = r.code == fsl.code == 0 and images(dirs["replica"]) == images(dirs["fsl"])
            row = {
                "run": run,
                "voxels": None,
                "fsl_seconds": round(fsl_s, 2),
                "fsl_served_from_cache": bool(tally.served),
                "replica_seconds": round(replica_s, 2),
                "replica_bit_identical": same,
                "original_seconds": {},
            }
            for n in a.threads:
                d = dirs[f"original{n}"]
                o0 = time.perf_counter()
                o = orig.run_original(args, cwd=d, settings={}, mode=defects.FIX, threads=n)
                row["original_seconds"][str(n)] = round(time.perf_counter() - o0, 2)
                assert o.code == 0 and set(images(d)) == set(images(dirs["fsl"])), (run, n)
            import nibabel as nib
            import numpy as np

            row["voxels"] = int((np.asanyarray(nib.load(str(src)).dataobj) > 0).sum())
            rows.append(row)
            times = " ".join(f"{k}:{v:.2f}" for k, v in row["original_seconds"].items())
            print(
                f"{run}: FSL {row['fsl_seconds']:.2f} s (cache {row['fsl_served_from_cache']}), "
                f"replica {replica_s:.2f} s (bit-identical {same}), original {times}"
            )
    if a.json:
        rev = subprocess.run(
            ["git", "rev-parse", "--short", "HEAD"], capture_output=True, text=True, check=False
        ).stdout.strip()
        a.json.write_text(
            json.dumps(
                {
                    "tool": "fsl fast (clean-room original)",
                    "commit": rev,
                    "machine": platform.machine(),
                    "load_average_before": round(load, 1),
                    "threads": a.threads,
                    "environment": "FSL's clean environment (FSL off the PATH, FSLOUTPUTTYPE=NIFTI_GZ, OPENBLAS_CORETYPE=Haswell); OMP_NUM_THREADS for the original",
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
