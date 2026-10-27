# SPDX-License-Identifier: Apache-2.0
"""Wall time and peak memory of fMRIPrep's whole-head registrations, replayed by the ANTs 2.6.5
oracle (one thread, as the panel record runs it) and by larmorx (reproduce mode), on a quiet
machine: the basis of the panel's estimate for antsRegistration (the brief's 12-hour bound).

    PYTHONPATH=<checkout>/python:<checkout>/validation/src \\
        python -P time_registration.py <out dir> <lx threads> <case id> [<case id> ...]

A case id is a panel case of the antsRegistration suite (``panel/fmriprep/<run>/<node>`` or its
``/small`` version). Each program runs as its own process under ``/usr/bin/time -v``, not
through the oracle cache (a time is wanted, not a result); ``<out dir>/timings.json`` collects
the numbers.
"""

from __future__ import annotations

import json
import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path


def timed(cmd: list[str], env: dict[str, str]) -> dict:
    p = subprocess.run(["/usr/bin/time", "-v", *cmd], capture_output=True, text=True, env=env,
                       check=False)  # fmt: skip
    rss = re.search(r"Maximum resident set size \(kbytes\): (\d+)", p.stderr)
    wall = re.search(r"Elapsed \(wall clock\) time \(h:mm:ss or m:ss\): ([\d:.]+)", p.stderr)
    seconds = 0.0
    if wall:
        for part in wall.group(1).split(":"):
            seconds = seconds * 60 + float(part)
    return {
        "exit": p.returncode,
        "seconds": seconds,
        "peak_mib": int(rss.group(1)) / 1024 if rss else None,
    }


def main() -> int:
    from larmorx_validation.parity import ants_registration as reg
    from larmorx_validation.parity.ants_programs import ants_bin

    out = Path(sys.argv[1])
    out.mkdir(parents=True, exist_ok=True)
    lx_threads = sys.argv[2]
    cases = {c.id: c for c in reg.all_cases()}
    rows = {}
    for cid in sys.argv[3:]:
        c = cases[cid]
        base = {k: v for k, v in os.environ.items() if k not in ("NSLOTS", "LD_PRELOAD")}
        base["ANTS_RANDOM_SEED"] = c.seed or reg.SEED
        row = {}
        with tempfile.TemporaryDirectory(dir=out) as tmp_name:
            tmp = Path(tmp_name)
            for label, head, threads in (
                ("ANTs", [str(ants_bin() / "antsRegistration")], "1"),
                ("larmorx", [sys.executable, "-m", "larmorx", "ants", "antsRegistration"],
                 lx_threads),
            ):  # fmt: skip
                d = tmp / label
                d.mkdir()
                args = reg._resolve(c, d, tmp)
                env = {**base, "ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS": threads}
                if label == "larmorx":
                    env["LARMORX_UPSTREAM_DEFECTS"] = "reproduce"
                row[label] = {**timed([*head, *args], env), "threads": int(threads)}
                print(cid, label, json.dumps(row[label]), flush=True)
        rows[cid] = row
        (out / "timings.json").write_text(json.dumps(rows, indent=1), encoding="utf-8")
    return 0


if __name__ == "__main__":
    sys.exit(main())
