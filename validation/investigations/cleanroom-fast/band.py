# SPDX-License-Identifier: Apache-2.0
"""The band of a last-bit change (``specs/fast.md`` §11): FSL 6.0.7.23's ``fast`` as built
against the same program with correctly rounded ``exp``, ``log``, ``expf`` and ``logf`` (the
workspace's CORE-MATH preload, ``oracles/core-math-preload/libcrmath.so``, through
``LD_PRELOAD``), both through the oracle cache, compared with the clean-room original's suite
metrics (``parity/fsl_fast.py``). It shows how far FSL's own outputs move when only the last
bits of its mathematical functions change: the allowance of an implementation that is not
bit-identical. A development tool of FAST's clean-room original (relay 1, 2026-11-03).

    PYTHONPATH=python:validation/src python -P validation/investigations/cleanroom-fast/band.py \
        fmriprep/ds000005-sub-01-noreconall opt/bias-outputs ...

Run it inside scripts/orchestration/boxed.sh.
"""

from __future__ import annotations

import shutil
import sys
import tempfile
from pathlib import Path

from larmorx_validation.parity import fsl_fast as orig
from larmorx_validation.parity import fsl_fast_replica as rep
from larmorx_validation.parity.harness import CheckList


def preload() -> str:
    from larmorx_validation.parity.fsl_mcflirt_replica import workspace

    ws = workspace()
    assert ws is not None
    return str(ws / "oracles" / "core-math-preload" / "libcrmath.so")


def main() -> None:
    for name in [a for a in sys.argv[1:] if not a.startswith("--")]:
        with (
            tempfile.TemporaryDirectory(prefix="lx-fast-band_") as a_,
            tempfile.TemporaryDirectory(prefix="lx-fast-band_") as b_,
        ):
            a, b = Path(a_), Path(b_)
            if name.startswith("fmriprep/"):
                run = name.split("/", 1)[1]
                node = next(n for r, n in rep.fmriprep_calls() if r == run)
                src = rep.fmriprep_input(run, str(node))
                args = rep.fmriprep_args(node)
                for d in (a, b):
                    shutil.copy(src, d / args[-1])
                settings: dict[str, str | None] = {}
                never = ""
            else:
                case = rep._all_cases()[name][2]
                settings = dict(case.settings)
                args = rep._prepare(case, a)
                rep._prepare(case, b)
                never = case.never
            p = rep.run_oracle(args, cwd=a, settings=settings, never=never)
            q = rep.run_oracle(
                args, cwd=b, settings={**settings, "LD_PRELOAD": preload()}, never=never
            )
            checks = CheckList()
            orig.compare_images(checks, a, b)
            print(f"== {name}: exit {p.code} / {q.code}")
            for c in checks.checks:
                print(f"   {'ok ' if c.passed else 'OUT'} {c.name[:40]}: {c.detail[:400]}")
            if "--original" in sys.argv:
                # the clean-room original (reproduce) against FSL with correctly rounded maths
                with tempfile.TemporaryDirectory(prefix="lx-fast-band_") as c_:
                    c = Path(c_)
                    if name.startswith("fmriprep/"):
                        shutil.copy(src, c / args[-1])
                    else:
                        rep._prepare(case, c)
                    r = orig.run_original(args, cwd=c, settings=settings, mode="reproduce")
                    same = [
                        f.name
                        for f in sorted(b.iterdir())
                        if (c / f.name).is_file() and _values(f) == _values(c / f.name)
                    ]
                    print(
                        f"   original vs FSL+CR: stdout {'same' if r.stdout == q.stdout else 'differs'}; values identical in {len(same)} files: {same[:8]}"
                    )
                    checks = CheckList()
                    orig.compare_images(checks, b, c)
                    for k in checks.checks:
                        print(f"   {'ok ' if k.passed else 'OUT'} {k.name[:40]}: {k.detail[:300]}")


def _values(p: Path) -> bytes:
    import nibabel as nib
    import numpy as np

    if not p.name.endswith((".nii", ".nii.gz", ".hdr")):
        return p.read_bytes()
    return np.asarray(nib.load(str(p)).dataobj).tobytes()


if __name__ == "__main__":
    main()
