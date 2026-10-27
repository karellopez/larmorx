# SPDX-License-Identifier: Apache-2.0
"""larmorx's peak memory and wall time on antsRegistration suite cases at one thread (ANTs' are
``time_registration.py``'s): the before/after numbers of the metric's chunks
(``docs/findings/ants-registration.md``, "Memory against ANTs (2026-10-26)").

    PYTHONPATH=<checkout>/python:<checkout>/validation/src \\
        python -P lx_registration_peak.py <out dir> <case id> ...
"""

from __future__ import annotations

import json
import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path


def main() -> int:
    from larmorx_validation.parity import ants_registration as reg

    out = Path(sys.argv[1])
    out.mkdir(parents=True, exist_ok=True)
    cases = {c.id: c for c in reg.all_cases()}
    skip = ("NSLOTS", "LD_PRELOAD", "RAYON_NUM_THREADS")
    for cid in sys.argv[2:]:
        c = cases[cid]
        env = {k: v for k, v in os.environ.items() if k not in skip}
        env.update(
            ANTS_RANDOM_SEED=c.seed or reg.SEED,
            ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS="1",
            LARMORX_UPSTREAM_DEFECTS="reproduce",
        )
        with tempfile.TemporaryDirectory(dir=out) as t:
            tmp = Path(t)
            d = tmp / "lx"
            d.mkdir()
            args = reg._resolve(c, d, tmp)
            cmd = [sys.executable, "-m", "larmorx", "ants", "antsRegistration", *args]
            p = subprocess.run(["/usr/bin/time", "-v", *cmd], capture_output=True, text=True,
                               env=env, check=False)  # fmt: skip
            rss = re.search(r"Maximum resident set size \(kbytes\): (\d+)", p.stderr)
            wall = re.search(
                r"Elapsed \(wall clock\) time \(h:mm:ss or m:ss\): ([\d:.]+)", p.stderr
            )
            row = {
                "exit": p.returncode,
                "peak_mib": int(rss.group(1)) / 1024 if rss else None,
                "wall": wall.group(1) if wall else None,
            }
            print(cid, json.dumps(row), flush=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
