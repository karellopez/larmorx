# SPDX-License-Identifier: Apache-2.0
"""catchup_schedule.py <inputs dir> <schedule> <outdir> <flirt words...>: FLIRT on a finding's
inputs (a directory of the files a random-testing case wrote, e.g. ``catchup_finding.py``'s
``fsl/``) with a schedule of the probe's own (its lines written from FSL's ``-v`` echo, with
``print`` lines added to see a list), FSL through the oracle cache (use mode, the inputs and the
schedule keyed) against the original in ``reproduce`` and in its default; standard output and
error saved as <outdir>/fsl.out, .err and <outdir>/<mode>.out, .err, and the first differing
lines printed. The clean-room catch-up, relay 4 (2026-11-05): the list a sort of all-NaN BBR
costs leaves first (finding 2424)."""

import difflib
import shutil
import sys
from pathlib import Path

from larmorx_validation import oracle_cache
from larmorx_validation.parity import defects
from larmorx_validation.parity import fsl_flirt as orig
from larmorx_validation.parity import fsl_flirt_replica as rep

src, schedule, root = Path(sys.argv[1]), Path(sys.argv[2]), Path(sys.argv[3])
words = [*sys.argv[4:], "-schedule", "probe.sch"]
inputs = [p for p in sorted(src.iterdir()) if p.is_file() and p.suffix in (".gz", ".nii", ".mat")]


def setup(d: Path) -> None:
    d.mkdir(parents=True, exist_ok=True)
    for p in inputs:
        if not p.name.startswith(("out", "refvol", "testvol", "global_", "debug")):
            shutil.copy(p, d / p.name)
    shutil.copy(schedule, d / "probe.sch")


def first_diff(a: str, b: str, n: int = 8) -> list[str]:
    return [
        ln
        for ln in difflib.unified_diff(a.splitlines(), b.splitlines(), lineterm="", n=0)
        if not ln.startswith(("---", "+++"))
    ][:n]


d = root / "fsl"
setup(d)
settings = {"FSLOUTPUTTYPE": "NIFTI_GZ"}
p = oracle_cache.run(
    [str(rep.fsldir() / "bin" / "flirt"), *words],
    env=rep._environment(settings),
    cwd=d,
    outputs=[d],
    policy=oracle_cache.Policy(threads=1, family="fsl"),
    runner="catchup-schedule",
    inputs=[x for x in sorted(d.iterdir()) if x.is_file()],
    timeout=1200,
)
(root / "fsl.out").write_text(p.stdout)
(root / "fsl.err").write_text(p.stderr)
print(f"FSL: exit {p.returncode}")
for mode in (defects.REPRODUCE, defects.FIX):
    m = root / mode
    setup(m)
    r = orig.run_original("flirt", words, cwd=m, settings=settings, mode=mode)
    (root / f"{mode}.out").write_text(r.stdout)
    (root / f"{mode}.err").write_text(r.stderr)
    print(f"original {mode}: exit {r.code}")
    for stream in ("stdout", "stderr"):
        diff = first_diff(getattr(p, stream), orig._own(getattr(r, stream)))
        if diff:
            print(f"  {stream} differs:")
            for ln in diff:
                print("    " + ln[:220])
