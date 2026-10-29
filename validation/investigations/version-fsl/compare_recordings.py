# SPDX-License-Identifier: Apache-2.0
"""mcflirt's two recordings, run by run: FSL 6.0.7.17 (``oracles/fsl-6.0.7/mcflirt``) against
FSL 6.0.7.23 (``oracles/fsl-6.0.7.23/mcflirt``, made by ``scripts/run_mcflirt_oracle.py
--fsl-version 6.0.7.23``): exit status, both streams (each recording's own directory and FSLDIR
masked) and every output file (images decompressed, ``descrip`` left out: it names the build).

Usage: python -P compare_recordings.py [<workspace>]
"""

from __future__ import annotations

import gzip
import json
import sys
from pathlib import Path


def blank_descrip(data: bytes) -> bytes:
    size = int.from_bytes(data[:4], "little")
    if size not in (348, 540):
        size = int.from_bytes(data[:4], "big")
    start = 240 if size == 540 else 148
    if len(data) < start + 80:
        return data
    return data[:start] + b"\0" * 80 + data[start + 80 :]


def content(p: Path) -> bytes:
    raw = p.read_bytes()
    if raw[:2] == b"\x1f\x8b":
        raw = gzip.decompress(raw)
    if p.name.endswith((".nii", ".nii.gz", ".hdr", ".hdr.gz")):
        raw = blank_descrip(raw)
    return raw


def main() -> int:
    ws = Path(sys.argv[1]) if len(sys.argv) > 1 else Path(__file__).resolve().parents[4]
    old = ws / "oracles" / "fsl-6.0.7" / "mcflirt"
    new = ws / "oracles" / "fsl-6.0.7.23" / "mcflirt"
    names = sorted(json.loads((old / "manifest.json").read_text())["runs"])
    changed = 0
    for name in names:
        a, b = old / "runs" / name, new / "runs" / name
        ra = json.loads((a / "run.json").read_text())
        rb = json.loads((b / "run.json").read_text())
        diffs = []
        ea = [s["exit"] for s in ra["steps"]]
        eb = [s["exit"] for s in rb["steps"]]
        if ea != eb:
            diffs.append(f"exit {ea} -> {eb}")
        for k in range(len(ra["steps"])):
            for stream in ("stdout", "stderr"):
                fa, fb = a / f"{stream}{k}.txt", b / f"{stream}{k}.txt"
                ta = fa.read_text(errors="replace").replace(str(a), "<run>") if fa.is_file() else ""
                tb = fb.read_text(errors="replace").replace(str(b), "<run>") if fb.is_file() else ""
                for d in (str(Path.home() / "fsl-6.0.7.23"), str(Path.home() / "fsl")):
                    ta, tb = ta.replace(d, "<FSLDIR>"), tb.replace(d, "<FSLDIR>")
                if ta != tb:
                    la, lb = ta.splitlines(), tb.splitlines()
                    i = next(
                        (i for i, (x, y) in enumerate(zip(la, lb, strict=False)) if x != y),
                        min(len(la), len(lb)),
                    )
                    x = la[i] if i < len(la) else "<end>"
                    y = lb[i] if i < len(lb) else "<end>"
                    diffs.append(f"{stream}{k} line {i + 1}: {x[:100]!r} -> {y[:100]!r}")
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
    print(f"{changed} of {len(names)} recorded runs changed between FSL 6.0.7.17 and 6.0.7.23")
    return 0


if __name__ == "__main__":
    sys.exit(main())
