# SPDX-License-Identifier: Apache-2.0
"""Runs the FSL 6.0.7.23 ``flirt`` measurements behind the FLIRT spec answers (2026-11-02), each
through the oracle-output cache (``fsl_flirt_replica.run_oracle``: a run is computed once), into
``<out>/<name>/`` with its standard output and error kept (``stdout.txt``, ``stderr.txt``).

    PYTHONPATH=python:validation/src scripts/orchestration/boxed.sh 8 \
        python -P validation/investigations/flirt-spec-answers/fsl_runs.py <out> [name ...]

Inputs: the ``fsl-flirt-replica`` suite's derived inputs (``oracles/inputs/fsl-flirt-replica/``,
made from ds005040's fMRIPrep run). The names:

- ``bbrcost``, ``bbrcost-noclamp``, ``bbrcost-local_abs``, ``bbrcost-global_abs``: the
  boundary-based cost of the initial matrix with ``bbrstep`` 1, 2, 200 and 201 (``bbrcost.sch``),
  the suite's ``bbr/default`` images (the default, ``-noclamp``, ``-bbrtype local_abs`` and
  ``global_abs``);
- ``dof6-trace``: the suite's ``real/dof6`` with ``-verbose 7`` (every cost evaluation);
- ``dof6-debug``: the same with ``-debug`` (the pyramid's levels and the 8 mm input:
  ``refvol_8``, ``testvol``);
- ``corratio-mc``: ``corratio_costs.sch``'s costs at 8 mm on ``real/dof6``'s images (the
  correlation ratio with smoothing 8 and 0, and Woods' cost, at two matrices);
- ``woods-neg``: the same with ``-debug`` on the BOLD reference minus 30000 (``epi_neg.nii``, a
  derived input of this part: bins whose sum of input values is not positive).
"""

from __future__ import annotations

import shutil
import sys
from pathlib import Path

from larmorx_validation.parity import fsl_flirt_replica as rep

HERE = Path(__file__).resolve().parent

BBR = (
    "-in",
    "@epi.nii",
    "-ref",
    "@t1.nii",
    "-init",
    "@init.mat",
    "-wmseg",
    "@wm.nii",
    "-cost",
    "bbr",
    "-omat",
    "o.mat",
    "-schedule",
    "bbrcost.sch",
)
DOF6 = ("-in", "@epi.nii", "-ref", "@t1.nii", "-dof", "6", "-omat", "out.mat")

RUNS: dict[str, tuple[tuple[str, ...], tuple[str, ...]]] = {
    # name: (arguments, schedule files copied from this directory)
    "bbrcost": (BBR, ("bbrcost.sch",)),
    "bbrcost-noclamp": ((*BBR, "-noclamp"), ("bbrcost.sch",)),
    "bbrcost-local_abs": ((*BBR, "-bbrtype", "local_abs"), ("bbrcost.sch",)),
    "bbrcost-global_abs": ((*BBR, "-bbrtype", "global_abs"), ("bbrcost.sch",)),
    "dof6-trace": ((*DOF6, "-verbose", "7"), ()),
    "dof6-debug": ((*DOF6, "-debug"), ()),
    "woods-neg": (
        (
            "-in",
            "@@epi_neg.nii",
            "-ref",
            "@t1.nii",
            "-omat",
            "o.mat",
            "-cost",
            "woods",
            "-schedule",
            "corratio_costs.sch",
            "-debug",
        ),
        ("corratio_costs.sch",),
    ),
    "corratio-mc": (
        (
            "-in",
            "@epi.nii",
            "-ref",
            "@t1.nii",
            "-omat",
            "o.mat",
            "-cost",
            "corratio",
            "-schedule",
            "corratio_costs.sch",
        ),
        ("corratio_costs.sch",),
    ),
}


def derived(name: str) -> Path:
    """This part's derived inputs, made once, deterministically: ``epi_neg.nii``, the suite's
    BOLD reference minus 30000 (so that the reference bins' sums of input values are negative
    where the input is dark: Woods' cost's other branch)."""
    import nibabel as nib
    import numpy as np

    ws = rep.workspace()
    assert ws is not None
    out = ws / "oracles" / "inputs" / "flirt-spec-answers"
    out.mkdir(parents=True, exist_ok=True)
    p = out / name
    if not p.is_file() and name == "epi_neg.nii":
        img = nib.load(str(rep.inputs() / "epi.nii"))
        a = np.asanyarray(img.dataobj).astype(np.float32) - np.float32(30000.0)
        h = img.header.copy()
        h.set_data_dtype(np.float32)
        nib.save(nib.Nifti1Image(a, img.affine, h), str(p))
    return p


def run(out: Path, name: str) -> None:
    args, schedules = RUNS[name]
    d = out / name
    shutil.rmtree(d, ignore_errors=True)
    d.mkdir(parents=True)
    for s in schedules:
        # this directory's schedules, else the clean-room part's probes' (bbrcost.sch)
        src = HERE / s if (HERE / s).is_file() else HERE.parent / "cleanroom-flirt" / s
        shutil.copy(src, d / s)
    # `@@name`: a derived input of this part (oracles/inputs/flirt-spec-answers/), copied in
    for a in args:
        if a.startswith("@@"):
            shutil.copy(derived(a[2:]), d / a[2:])
    args = tuple(a[2:] if a.startswith("@@") else a for a in args)
    argv = rep._prepare(rep.Run("flirt", args), d)
    p = rep.oracle_run("flirt", argv, cwd=d, settings={})
    (d / "stdout.txt").write_text(p.stdout, encoding="utf-8")
    (d / "stderr.txt").write_text(p.stderr, encoding="utf-8")
    cached = getattr(p, "cached", None)
    print(
        f"{name}: exit {p.returncode}, cached {cached}, stdout {len(p.stdout.splitlines())} lines"
    )


def main() -> None:
    out = Path(sys.argv[1])
    for name in sys.argv[2:] or list(RUNS):
        run(out, name)


if __name__ == "__main__":
    main()
