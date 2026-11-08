# SPDX-License-Identifier: Apache-2.0
"""AFNI 26.3.05's 3dvolreg as a black box, through the oracle-output cache.

``run(args, cwd)`` runs ``3dvolreg <args>`` in ``cwd`` with a clean environment (``HOME`` an empty
folder, ``AFNI_DONT_LOGFILE=YES``, ``AFNI_NIFTI_TYPE_WARN=NO``, one thread), served from
``<workspace>/oracles/cache`` when the same program, inputs, arguments and settings ran before.
Written from the spec's validation plan (``specs/3dvolreg.md`` §14.1) and AFNI's documentation:
no AFNI source. ``AFNI_VOLREG`` names another 3dvolreg.
"""

from __future__ import annotations

import os
from dataclasses import dataclass
from pathlib import Path

WORKSPACE = Path(os.environ.get("LARMORX_WORKSPACE", Path(__file__).resolve().parents[4]))
AFNI = Path(os.environ.get("AFNI_VOLREG", WORKSPACE / "oracles/afni-26.3.05/bin/3dvolreg"))


@dataclass
class Run:
    status: int | None
    stdout: str
    stderr: str


def run(args, cwd: Path, extra_env: dict[str, str] | None = None) -> Run:
    """``3dvolreg <args>`` in ``cwd`` (created), through the oracle cache."""
    from larmorx_validation import oracle_cache

    cwd.mkdir(parents=True, exist_ok=True)
    home = cwd.parent / "_home"
    home.mkdir(exist_ok=True)
    env = {
        "HOME": str(home),
        "PATH": "/usr/bin:/bin",
        "AFNI_DONT_LOGFILE": "YES",
        "AFNI_NIFTI_TYPE_WARN": "NO",
        "OMP_NUM_THREADS": "1",
        **(extra_env or {}),
    }
    p = oracle_cache.run(
        [str(AFNI), *map(str, args)],
        env=env,
        cwd=cwd,
        policy=oracle_cache.Policy(threads=1, family="afni"),
        runner="3dvolreg-spec",
    )
    return Run(
        p.returncode,
        p.stdout_bytes.decode("utf-8", "replace"),
        p.stderr_bytes.decode("utf-8", "replace"),
    )


def params(path: Path) -> list[list[float]]:
    """The six reported parameters per volume of a ``-1Dfile``."""
    rows = []
    for line in Path(path).read_text(encoding="utf-8").splitlines():
        if line.strip() and not line.startswith("#"):
            rows.append([float(v) for v in line.split()])
    return rows
