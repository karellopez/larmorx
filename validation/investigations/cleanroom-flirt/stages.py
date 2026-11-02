# SPDX-License-Identifier: Apache-2.0
"""Where a registration's path leaves FSL's: the default 3D schedule (the original's copy of the
lines `flirt -v` prints) with `print` commands after the search and after every `measurecost`
and `optimise`, run by FSL 6.0.7.23 (through the oracle cache) and the clean-room original on a
suite case's images; prints, stage by stage, the largest difference of the printed rows (cost
and matrix) between the two. A development tool of the clean-room original (part 1, 2026-11-02).

    PYTHONPATH=python:validation/src LARMORX_FLIRT_BIN=target/release/larmorx \\
        python -P validation/investigations/cleanroom-flirt/stages.py <work dir> [flirt args]

Run it inside scripts/orchestration/boxed.sh.
"""

from __future__ import annotations

import re
import shlex
import sys
from pathlib import Path

import numpy as np
from larmorx_validation.parity import defects
from larmorx_validation.parity import fsl_flirt as orig
from larmorx_validation.parity import fsl_flirt_replica as rep

RUST = Path(__file__).resolve().parents[3] / "crates/larmorx-fsl/src/flirt/schedule.rs"


def default_schedule() -> list[str]:
    text = RUST.read_text(encoding="utf-8")
    body = text.split('pub const DEFAULT_3D: &str = "', 1)[1].split('";', 1)[0]
    return body.replace("\\x20", " ").splitlines()


def instrumented() -> str:
    out = []
    for ln in default_schedule():
        out.append(ln)
        w = ln.split()
        if not w or w[0].startswith("#"):
            continue
        if w[0] == "search":
            out += ["print S", "print P"]
        elif w[0] in ("optimise", "measurecost"):
            out.append("print U")
    return "\n".join(out) + "\n"


def rows(text: str) -> list[list[float]]:
    return [
        [float(v) for v in ln.split()]
        for ln in text.splitlines()
        if ln.strip() and re.fullmatch(r"[-+0-9.eEna ]+", ln.strip())
    ]


def main() -> None:
    work = Path(sys.argv[1])
    extra = sys.argv[2:] or ["-dof", "6"]
    sch = instrumented()
    args = ("-in", "@epi.nii", "-ref", "@t1.nii", "-omat", "o.mat", "-schedule", "s.sch", *extra)
    run = rep.Run("flirt", args, files=(("s.sch", sch),))
    fd, od = work / "fsl", work / "orig"
    for d in (fd, od):
        d.mkdir(parents=True, exist_ok=True)
    a = rep._prepare(run, fd)
    rep._prepare(run, od)
    f = rep.run_oracle("flirt", a, cwd=fd, settings={})
    o = orig.run_original("flirt", a, cwd=od, settings={}, mode=defects.REPRODUCE)
    print("args:", shlex.join(a), "| exit", f.code, o.code)
    (work / "FSL.stdout").write_text(f.stdout, encoding="utf-8")
    (work / "original.stdout").write_text(o.stdout, encoding="utf-8")
    rf, ro = rows(f.stdout), rows(o.stdout)
    print("rows", len(rf), len(ro))
    for i, (x, y) in enumerate(zip(rf, ro, strict=False)):
        x, y = np.array(x), np.array(y)
        if x.shape != y.shape:
            print(i, "shape", x.shape, y.shape)
            continue
        d = np.max(np.abs(x - y)) if x.size else 0.0
        if d > 1e-6:
            print(f"row {i}: max |difference| {d:.3g}  FSL {x[:5]}  original {y[:5]}")


if __name__ == "__main__":
    main()
