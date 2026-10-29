# SPDX-License-Identifier: Apache-2.0
"""FSL 6.0.7.23's two mcflirt recordings, run by run: on OpenBLAS's own choice of core for this
CPU (``oracles/fsl-6.0.7.23/mcflirt``, the first recording: Cooperlake on an AMD Zen 4) against
the Haswell core the validation pins (``oracles/fsl-6.0.7.23/mcflirt-haswell``,
``scripts/run_mcflirt_oracle.py --fsl-version 6.0.7.23``): exit status, both streams (each
recording's own directory masked) and every output file (images decompressed). The same binary
and inputs; only the kernels of Armadillo 14.2.1's LAPACK path differ (FSL-64).

Usage: python -P compare_cores.py [<workspace>]
"""

from __future__ import annotations

import gzip
import json
import sys
from pathlib import Path


def content(p: Path) -> bytes:
    raw = p.read_bytes()
    return gzip.decompress(raw) if raw[:2] == b"\x1f\x8b" else raw


def main() -> int:
    ws = Path(sys.argv[1]) if len(sys.argv) > 1 else Path(__file__).resolve().parents[4]
    old = ws / "oracles" / "fsl-6.0.7.23" / "mcflirt"
    new = ws / "oracles" / "fsl-6.0.7.23" / "mcflirt-haswell"
    names = sorted(json.loads((old / "manifest.json").read_text())["runs"])
    changed = 0
    for name in names:
        a, b = old / "runs" / name, new / "runs" / name
        ra = json.loads((a / "run.json").read_text())
        rb = json.loads((b / "run.json").read_text())
        diffs = []
        if [s["exit"] for s in ra["steps"]] != [s["exit"] for s in rb["steps"]]:
            diffs.append("exit status")
        for k in range(len(ra["steps"])):
            for stream in ("stdout", "stderr"):
                fa, fb = a / f"{stream}{k}.txt", b / f"{stream}{k}.txt"
                ta = fa.read_text(errors="replace").replace(str(a), "<run>") if fa.is_file() else ""
                tb = fb.read_text(errors="replace").replace(str(b), "<run>") if fb.is_file() else ""
                if ta != tb:
                    diffs.append(f"{stream}{k}")
        oa, ob = set(ra.get("outputs", {})), set(rb.get("outputs", {}))
        skip = {"run.json"}
        if oa - skip != ob - skip:
            diffs.append(f"files {sorted((oa ^ ob) - skip)[:5]}")
        for f in sorted((oa & ob) - skip):
            if f.startswith(("stdout", "stderr")):
                continue
            if content(a / f) != content(b / f):
                diffs.append(f"{f} differs")
        if diffs:
            changed += 1
            print(f"CHANGED {name}: " + "; ".join(diffs[:6]))
    print(f"{changed} of {len(names)} recorded runs differ between OpenBLAS's own core and Haswell")
    return 0


if __name__ == "__main__":
    sys.exit(main())
