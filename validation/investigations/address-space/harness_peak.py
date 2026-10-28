# SPDX-License-Identifier: Apache-2.0
"""The harness's own peak virtual size while larmorx runs in-process on a suite's largest cases
(``ants_programs.run_larmorx`` runs under :func:`larmorx_validation.address_space.limited`, and
the harness's own address space counts toward the limit there).

Usage (from the checkout, the environment of ``scripts/orchestration/env.sh``, in a memory box)::

    python -P harness_peak.py <out> <suite> <tier> <case regex>

Runs ``python -m larmorx_validation parity <suite> --tier <tier> --case <regex> --out <out>`` in
this process, then prints this process's ``VmPeak`` (the largest virtual size over its whole
life: the interpreter, numpy, the harness and every larmorx run) and ``VmHWM``.
"""

from __future__ import annotations

import sys
from pathlib import Path


def main(argv: list[str] | None = None) -> int:
    from larmorx_validation.__main__ import main as lxv

    out, suite, tier, case = argv or sys.argv[1:]
    status = lxv(["parity", suite, "--tier", tier, "--case", case, "--out", out])
    fields = {}
    for line in Path("/proc/self/status").read_text().splitlines():
        name, _, value = line.partition(":")
        if name in ("VmPeak", "VmHWM", "Threads"):
            fields[name] = value.strip()
    print(
        f"HARNESS-PEAK {suite} {case}: VmPeak {fields.get('VmPeak')}, VmHWM {fields.get('VmHWM')}"
    )
    return status


if __name__ == "__main__":
    sys.exit(main())
