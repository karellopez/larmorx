# SPDX-License-Identifier: Apache-2.0
"""makerot with NaN and infinite angles, axes and centres: FSL 6.0.7.23's printed matrix (which
entries print ``nan`` and which ``-nan``) against the replica's (``LARMORX_NC_BIN``), FLIRT part 3
(2026-11-01). FSL runs once per call; its outputs are kept in ``<out>/fsl-<k>.txt`` and reused
when present (``--fresh`` runs FSL again).

Usage::

    LARMORX_NC_BIN=<larmorx-nc> python -P makerot_nan.py <out> [--fresh]
"""

from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path

FSL = Path.home() / "fsl-6.0.7.23"
CALLS = [
    ["-t", "nan"],
    ["-t", "inf", "-a", "1,0,0"],
    ["-t", "30", "-a", "nan,0,1"],
    ["-t", "30", "-a", "1,nan,0"],
    ["-t", "30", "-a", "0,0,1", "-c", "nan,0,0"],
    ["-t", "-inf"],
    ["-t", "30", "-a", "inf,0,0"],
    ["-t", "-nan", "-a", "0,1,0"],
    ["-t", "30", "-a", "0,0,-nan"],
    ["-t", "inf", "-a", "0,0,1", "-c", "1,2,3"],
]


def main() -> int:
    out = Path(sys.argv[1])
    out.mkdir(parents=True, exist_ok=True)
    fresh = "--fresh" in sys.argv
    env = dict(os.environ, FSLDIR=str(FSL), LARMORX_UPSTREAM_DEFECTS="reproduce")
    nc = os.environ["LARMORX_NC_BIN"]
    bad = 0
    for k, args in enumerate(CALLS):
        kept = out / f"fsl-{k}.txt"
        if fresh or not kept.is_file():
            p = subprocess.run(
                [str(FSL / "bin" / "makerot"), *args],
                env=env,
                capture_output=True,
                text=True,
                check=False,
            )
            kept.write_text(f"{p.returncode}\n{p.stdout}", encoding="utf-8")
        fsl = kept.read_text(encoding="utf-8")
        r = subprocess.run(
            [nc, "fsl", "makerot", *args], env=env, capture_output=True, text=True, check=False
        )
        rep = f"{r.returncode}\n{r.stdout}"
        same = fsl == rep
        bad += not same
        print(f"[{k}] makerot {' '.join(args)}: {'identical' if same else 'DIFFER'}")
        if not same:
            print("  FSL:", fsl.replace("\n", " | "))
            print("  REP:", rep.replace("\n", " | "))
    print(f"{len(CALLS)} calls, {bad} differ")
    return 0 if bad == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
