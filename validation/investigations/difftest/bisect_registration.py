# SPDX-License-Identifier: Apache-2.0
"""Where an antsRegistration finding of the differential tests starts to differ: its shrunk inputs
rebuilt, then ANTs with CORE-MATH preloaded and larmorx in ``reproduce`` mode (one thread,
``ANTS_RANDOM_SEED`` of the case) on the finding's arguments with edits, each run's first
differing ``DIAGNOSTIC`` line of standard output (the per-iteration metric values) and the
output files' differences.

    python -P validation/investigations/difftest/bisect_registration.py <finding.json> <dir> \
        ["<old>=><new>" ...]

Each edit replaces the text ``<old>`` of the argument line with ``<new>`` (``-v 1`` is kept);
``re:<pattern>`` replaces the first match of a regular expression.
"""

from __future__ import annotations

import dataclasses
import json
import re
import sys
from pathlib import Path

import nibabel as nib
import numpy as np

from larmorx_validation.difftest import tools
from larmorx_validation.parity import ants_registration as reg

ADDRESS = re.compile(r"0x[0-9a-f]+")


def diagnostics(text: str) -> list[str]:
    """The DIAGNOSTIC lines without their two time columns (the iteration's and the total)."""
    out = []
    for ln in text.splitlines():
        if "DIAGNOSTIC" in ln:
            fields = [f.strip() for f in ADDRESS.sub("0x", ln).split(",")]
            out.append(", ".join(fields[:4]))
    return out


def compare_dirs(a: Path, b: Path) -> list[str]:
    out = []
    for f in sorted(a.glob("*.nii.gz")):
        g = b / f.name
        if not g.is_file():
            out.append(f"{f.name}: larmorx wrote none")
            continue
        x, y = np.asarray(nib.load(f).dataobj), np.asarray(nib.load(g).dataobj)
        if x.shape != y.shape:
            out.append(f"{f.name}: shapes {x.shape} {y.shape}")
            continue
        d = np.abs(x.astype(np.float64) - y.astype(np.float64))
        n = int(np.count_nonzero(d))
        out.append(f"{f.name}: {n} of {d.size} values differ" + (f" (max {d.max():.3g})" if n else ""))
        for idx in list(zip(*np.nonzero(d), strict=True))[:4]:
            out.append(f"    at {tuple(int(i) for i in idx)}: ANTs {x[idx]!r}, larmorx {y[idx]!r}")
    return out


def main() -> int:
    finding = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
    work = Path(sys.argv[2])
    edits = [e.split("=>", 1) for e in sys.argv[3:]]
    case = tools.get("ants-registration").case(finding.get("shrunk") or finding["recipe"], "port")
    c = case.payload
    line = " ".join(c.args)
    for old, new in edits:
        if old.startswith("re:"):
            line, n = re.subn(old[3:], new, line, count=1)
            if not n:
                raise SystemExit(f"{old!r} matches nothing in the arguments")
            continue
        if old not in line:
            raise SystemExit(f"{old!r} is not in the arguments")
        line = line.replace(old, new)
    c = dataclasses.replace(c, args=tuple(line.split()))
    tag = re.sub(r"[^A-Za-z0-9]+", "_", "_".join(n for _, n in edits))[:60] or "as-found"
    a_dir, l_dir = work / tag / "ants", work / tag / "larmorx"
    for d in (a_dir, l_dir):
        d.mkdir(parents=True, exist_ok=True)
        for f in d.iterdir():
            if f.is_file():
                f.unlink()  # this tool's own outputs of an earlier run with the same edits
    a = reg._run_ants(reg._resolve(c, a_dir, work), c.seed, correctly_rounded=True, never="inspection")
    lx = reg._run_larmorx(reg._resolve(c, l_dir, work), c.seed)
    print("args:", line)
    print(f"ANTs (CORE-MATH) exit {a[0]}; larmorx exit {lx[0]}")
    da, dl = diagnostics(a[1]), diagnostics(lx[1])
    first = next((i for i, (x, y) in enumerate(zip(da, dl, strict=False)) if x != y), None)
    if first is None and len(da) == len(dl):
        print(f"{len(da)} DIAGNOSTIC lines, all identical")
    else:
        i = first if first is not None else min(len(da), len(dl))
        print(f"DIAGNOSTIC line {i + 1} of {len(da)} (larmorx {len(dl)}) differs:")
        print("  ANTs    " + (da[i] if i < len(da) else "(none)"))
        print("  larmorx " + (dl[i] if i < len(dl) else "(none)"))
    for s in compare_dirs(a_dir, l_dir):
        print("  " + s)
    return 0


if __name__ == "__main__":
    sys.exit(main())
