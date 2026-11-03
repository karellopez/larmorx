# SPDX-License-Identifier: Apache-2.0
"""The images whose robust range FSL's programs take through ``find_thresholds``, one path per
line, for ``crates-nc/larmorx-nc-core/tests/robust_range_probe.rs`` (FSL-187's measurement,
rule D): fMRIPrep's ``bet`` calls (the BOLD references of the oracle runs) and ``flirt -bbr``
calls (their ``-in`` and ``-ref``), the BET suite's real images (``larmorx-testdata``), and every
input of the fslmaths, fslstats, FLIRT and FAST replica suites (``<workspace>/oracles/inputs``).

Run: ``python -P robust_range_inputs.py > <scratch>/robust_range_inputs.txt``
(``PYTHONPATH`` with ``validation/src``)."""

from __future__ import annotations

import contextlib

from larmorx_validation.parity import fsl_bet, fsl_flirt_replica
from larmorx_validation.parity.fsl_bet import workspace


def main() -> None:
    paths: list[str] = []
    for _cid, source, _node in fsl_bet._fmriprep_calls():
        paths.append(str(source))
    for run, node in fsl_flirt_replica.fmriprep_calls():
        args = fsl_flirt_replica.fmriprep_args(run, node)
        for flag in ("-in", "-ref"):
            if flag in args:
                paths.append(args[args.index(flag) + 1])
    for rel in fsl_bet.REAL_FILES.values():
        with contextlib.suppress(FileNotFoundError):
            paths.append(str(fsl_bet._testdata(rel)))
    ws = workspace()
    if ws is not None:
        for suite in (
            "fsl-maths-replica",
            "fsl-stats-replica",
            "fsl-flirt-replica",
            "fsl-fast-replica",
        ):
            root = ws / "oracles" / "inputs" / suite
            paths += sorted(str(p) for p in root.rglob("*.nii*") if p.is_file())
    seen = set()
    for p in paths:
        if p not in seen:
            seen.add(p)
            print(p)


if __name__ == "__main__":
    main()
