# SPDX-License-Identifier: Apache-2.0
"""getFinalVox2Vox as the official build computes it, on the port's three matrices of a run.

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (it names FreeSurfer's functions; running fsrobust is a
black-box run of FreeSurfer's code).

Relay 45 of track C (2026-11-23): the fs-robust probe's ``v2v`` command (``fsrobust.cpp``, built by
``validation/probes/fs-robust/build.sh <workspace> <workspace>/oracles/freesurfer-8.2.0-probes/
fs-robust-v2v``) runs ``Registration::getFinalVox2Vox`` compiled as the official program is, on
Rtrg, Mfinal and Rsrc given as their bits (``v2v.py``'s ``DUMP``, or a file of ``LXDBG`` lines),
through the oracle-output cache at one thread, and prints the product with 13 decimals (the
official program prints ``vnl_matlab_print``'s long format), ``vnl_inverse(Rsrc)`` and ``Rtrg *
Mfinal`` (bits).

    scripts/orchestration/boxed.sh 4 python -P validation/probes/fs-robust-register/v2v_official.py \\
        [<file with LXDBG lines>]
"""

from __future__ import annotations

import struct
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parents[1] / "src"))

from larmorx_validation import oracle_cache  # noqa: E402
from larmorx_validation.oracle_cache.store import workspace  # noqa: E402


def h(x: str) -> float:
    return struct.unpack(">d", bytes.fromhex(x))[0]


def main(argv: list[str]) -> int:
    src = (HERE / "v2v.py").read_text(encoding="utf-8")
    dump = src.split('DUMP = """', 1)[1].split('"""', 1)[0]
    text = Path(argv[0]).read_text(encoding="utf-8") if argv else dump
    mats = {}
    for line in text.splitlines():
        if line.startswith("LXDBG"):
            t = line.split()
            mats[t[1]] = t[2:18]
    ws = workspace()
    if ws is None:
        raise SystemExit("no workspace with oracles/")
    exe = ws / "oracles" / "freesurfer-8.2.0-probes" / "fs-robust-v2v" / "fsrobust"
    inputs = ws / "oracles" / "inputs" / "fs-robust-register"
    inputs.mkdir(parents=True, exist_ok=True)
    case = inputs / "v2v-cases.txt"
    case.write_text(
        "v2v " + " ".join(mats["rtrg"] + mats["mfinal"] + mats["rsrc"]) + "\n", encoding="utf-8"
    )
    env = {"PATH": "/usr/bin:/bin", "LC_ALL": "C", "LANG": "C", "OMP_NUM_THREADS": "1"}
    done = oracle_cache.run(
        [exe, case],
        env=env,
        policy=oracle_cache.Policy(threads=1, family="freesurfer"),
        timeout=600,
        runner="fs-robust-register-v2v",
    )
    if done.returncode != 0:
        raise SystemExit(f"fsrobust failed ({done.returncode}): {done.stderr[-2000:]}")
    result = done.stdout.splitlines()[0].partition(" | ")[0].split()
    vals = [h(x) for x in result]
    names = ("getFinalVox2Vox", "vnl_inverse(Rsrc)", "Rtrg * Mfinal")
    for k, name in enumerate(names):
        print(f"{name} (official build):")
        for r in range(4):
            row = vals[16 * k + 4 * r : 16 * k + 4 * r + 4]
            print(
                "  "
                + " ".join(f"{v:.13f}" for v in row)
                + "   "
                + " ".join(result[16 * k + 4 * r : 16 * k + 4 * r + 4])
            )
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
