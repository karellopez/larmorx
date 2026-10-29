# SPDX-License-Identifier: Apache-2.0
"""The peak memory of AFNI 26.3.05's 3dTshift, 3dAutomask and 3dUnifize against the AFNI
replicas on the inputs of ``tests/parity/test_memory_peak.py`` (``AFNI_CASES``): the replica
through larmorx's Python entry point (``python -m larmorx``, in-process since 2026-10-29), the
standalone ``larmorx`` binary (no Python), and, when given, the ``larmorx-gpl`` program the
replicas were until then (a build of an earlier checkout). One thread; ``/usr/bin/time -v``'s
maximum resident set size. Prints a Markdown table.

Usage (from the checkout, with the built ``_core`` and ``cargo build --release -p larmorx-cli``)::

    PYTHONPATH=python:validation/src:tests/parity python -P \\
        validation/investigations/afni-move/memory.py [--gpl <larmorx-gpl>]
"""

from __future__ import annotations

import argparse
import os
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    p.add_argument("--gpl", help="an earlier build's larmorx-gpl program")
    a = p.parse_args()
    sys.path.insert(0, str(ROOT / "tests" / "parity"))
    import test_memory_peak as t
    from larmorx_validation.parity import afni_replica as ar

    standalone = ROOT / "target" / "release" / "larmorx"
    print(
        "| Program | Arguments | AFNI (MiB) | python -m larmorx (MiB) | larmorx binary (MiB) "
        "| larmorx-gpl (MiB) |"
    )
    print("|---|---|---|---|---|---|")
    with tempfile.TemporaryDirectory(prefix="lx-afni-mem-") as tmp_name:
        tmp = Path(tmp_name)
        (tmp / "in").mkdir()
        paths = t._afni_inputs(tmp / "in")
        home = tmp / "home"
        home.mkdir()
        env = {
            **{k: v for k, v in os.environ.items() if not k.upper().startswith(("AFNI_", "NIML_"))},
            "OMP_NUM_THREADS": "1",
            "HOME": str(home),
            "AFNI_DONT_LOGFILE": "YES",
            "AFNI_NIFTI_NOEXT": "YES",
            "LARMORX_UPSTREAM_DEFECTS": "reproduce",
            "LARMORX_IMPLEMENTATION": "replica",
        }
        for i, case in enumerate(t.AFNI_CASES):
            program, args = case.values if hasattr(case, "values") else case
            row = []
            runs = [
                ("afni", [ar.afni_binary(program, "26.3.05")]),
                ("python", [sys.executable, "-m", "larmorx", "afni", program]),
                ("binary", [str(standalone), "afni", program]),
            ]
            if a.gpl:
                runs.append(("gpl", [a.gpl, "afni", program]))
            for label, cmd in runs:
                out = tmp / f"{i}-{label}"
                out.mkdir()
                argv = [w.format(**paths, out=out) for w in args.split()]
                code, mib = t._peak([*cmd, *argv], env, out)
                row.append(f"{mib:.1f}" if code == 0 else f"exit {code}")
            if not a.gpl:
                row.append("-")
            print(f"| {program} | `{args}` | " + " | ".join(row) + " |", flush=True)
    print("\nMEMORY-DONE")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
