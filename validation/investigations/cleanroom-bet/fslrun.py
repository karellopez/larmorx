# SPDX-License-Identifier: Apache-2.0
"""Black-box runs of FSL's BET programs for the clean-room original (``specs/bet.md``).

``python -P fslrun.py <dir> <program> <args...>`` runs ``~/fsl-<version>/bin/<program>`` in
``<dir>`` (created) in a clean environment: ``PATH=/usr/bin:/bin`` (FSL never on it),
``FSLDIR`` set, an empty ``HOME``, ``FSLOUTPUTTYPE`` from the environment variable ``OT``
(default ``NIFTI_GZ``), ``LC_ALL=C``. It prints the exit status and both streams. ``--lx`` runs
``larmorx fsl <program>`` (the clean-room original, ``LARMORX_IMPLEMENTATION=original``) from
``LARMORX_BIN`` instead, in the same environment. Only the programs' behaviour is observed;
FSL's source is never read.
"""

from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path

FSLDIR = Path(os.environ.get("LARMORX_FSLDIR", Path.home() / "fsl-6.0.7.23"))


def environment(output_type: str | None = None, home: Path | None = None) -> dict[str, str]:
    env = {
        "HOME": str(home or Path(os.environ.get("TMPDIR", "/tmp"))),
        "PATH": "/usr/bin:/bin",
        "FSLDIR": str(FSLDIR),
        "LC_ALL": "C",
        "OMP_NUM_THREADS": "1",
        "OPENBLAS_CORETYPE": "Haswell",
    }
    ot = output_type if output_type is not None else os.environ.get("OT", "NIFTI_GZ")
    if ot:
        env["FSLOUTPUTTYPE"] = ot
    return env


def run(
    program: str,
    args: list[str],
    cwd: Path,
    *,
    larmorx: bool = False,
    extra: dict[str, str] | None = None,
) -> subprocess.CompletedProcess[str]:
    cwd.mkdir(parents=True, exist_ok=True)
    env = environment()
    env.update(extra or {})
    if larmorx:
        binary = os.environ.get("LARMORX_BIN", "larmorx")
        env["LARMORX_IMPLEMENTATION"] = "original"
        env.update({k: v for k, v in os.environ.items() if k.startswith("LARMORX_UPSTREAM")})
        command = [binary, "fsl", program, *args]
    else:
        command = [str(FSLDIR / "bin" / program), *args]
    return subprocess.run(
        command, cwd=cwd, env=env, capture_output=True, text=True, errors="replace", check=False
    )


def main(argv: list[str]) -> int:
    larmorx = "--lx" in argv
    argv = [a for a in argv if a != "--lx"]
    d, program, *args = argv
    p = run(program, args, Path(d), larmorx=larmorx)
    print(f"exit {p.returncode}")
    print("--- stdout"), print(p.stdout, end="")
    print("--- stderr"), print(p.stderr, end="")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
