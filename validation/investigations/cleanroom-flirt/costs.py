# SPDX-License-Identifier: Apache-2.0
"""FSL's and the clean-room original's costs of the same matrices, side by side: `costs.sch` (or
another schedule) measures two matrices at the 8, 4 and 2 mm pairs and with smoothing 0 and fuzzy
fraction 0, printing each row; this prints the costs (the rows' first numbers) of FSL 6.0.7.23
(through the oracle cache) and of the original, for every cost function, on the fsl-flirt suite's
ds005040 images. A development tool of the clean-room original (part 1, 2026-11-02).

    PYTHONPATH=python:validation/src LARMORX_FLIRT_BIN=target/release/larmorx \\
        python -P validation/investigations/cleanroom-flirt/costs.py [--schedule S] [-- extra args]

Run it inside scripts/orchestration/boxed.sh.
"""

from __future__ import annotations

import argparse
import tempfile
from pathlib import Path

from larmorx_validation.parity import defects
from larmorx_validation.parity import fsl_flirt as orig
from larmorx_validation.parity import fsl_flirt_replica as rep

HERE = Path(__file__).resolve().parent
COSTS = ("corratio", "normcorr", "leastsq", "woods", "mutualinfo", "normmi", "labeldiff")


def first_column(text: str) -> list[str]:
    return [ln.split()[0] for ln in text.splitlines() if ln.strip()]


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--schedule", default=str(HERE / "costs.sch"))
    ap.add_argument("--costs", default=",".join(COSTS))
    ap.add_argument("extra", nargs="*")
    a = ap.parse_args()
    for cost in a.costs.split(","):
        args = ("-in", "@epi.nii", "-ref", "@t1.nii", "-omat", "out.mat", "-schedule", a.schedule,
                "-cost", cost, *a.extra)  # fmt: skip
        run = rep.Run("flirt", tuple(args))
        with tempfile.TemporaryDirectory() as fd_, tempfile.TemporaryDirectory() as od_:
            fd, od = Path(fd_), Path(od_)
            words = rep._prepare(run, fd)
            rep._prepare(run, od)
            f = rep.run_oracle("flirt", words, cwd=fd, settings={})
            o = orig.run_original("flirt", words, cwd=od, settings={}, mode=defects.REPRODUCE)
        pairs = zip(first_column(f.stdout), first_column(o.stdout), strict=False)
        print(f"{cost}: " + "  ".join(f"{x}/{y}" for x, y in pairs))


if __name__ == "__main__":
    main()
