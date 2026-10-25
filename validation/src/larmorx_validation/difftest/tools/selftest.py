# SPDX-License-Identifier: Apache-2.0
"""A tool that tests the harness itself, never part of ``--tools all``: its "original" (a child
process) or its "larmorx" (in the worker, as larmorx runs) fills memory beyond the night's limit
when the case says so, and touches it page by page as a runaway program does. The memory limits
(:mod:`~larmorx_validation.difftest.limits`) must record such a case with the verdict ``memory``
and let the night go on (``validation/tests/test_difftest.py``).

    python -m larmorx_validation difftest --tools selftest-memory --max-cases 8 --jobs 1 \\
        --memory-gib 0.75 --box-gib 2 --out <dir>
"""

from __future__ import annotations

import subprocess
import sys

from larmorx_validation.difftest import gen
from larmorx_validation.difftest.core import Tool

#: Who fills memory in a case: nobody, the original (a child process) or larmorx (the worker).
WHO = ("nobody", "original", "larmorx")
#: How much a case fills, in MiB: a little, or more than any limit a test sets.
SMALL_MIB = 8
LARGE_MIB = 4096

FILL = "import sys\nx = bytearray(int(sys.argv[1]) << 20)\nfor i in range(0, len(x), 4096):\n    x[i] = 1\n"


def fill(mib: int) -> int:
    """Touches ``mib`` MiB, page by page (in this process); returns how many pages."""
    x = bytearray(mib << 20)
    for i in range(0, len(x), 4096):
        x[i] = 1
    return len(x) // 4096


def draw(d: gen.Draw) -> dict:
    who = d.choice("who", WHO)
    return {"groups": [], "inputs": {}, "options": [f"{who} fills memory"], "extra": {"who": who}}


def case(recipe: dict, impl: str, **kw):
    from larmorx_validation.parity.harness import Case

    return Case(
        f"selftest-{recipe['extra']['who']}", "difftest", recipe["options"][0], recipe["extra"]
    )


def run(impl: str):
    def run_case(c, checks) -> None:
        who = c.payload["who"]
        p = subprocess.run(
            [sys.executable, "-c", FILL, str(LARGE_MIB if who == "original" else SMALL_MIB)],
            capture_output=True,
            check=False,
        )
        checks("original ran", p.returncode == 0, detail=f"exit {p.returncode}")
        pages = fill(LARGE_MIB if who == "larmorx" else SMALL_MIB)
        checks("larmorx ran", pages > 0)

    return run_case


MEMORY_HOG = Tool(
    name="selftest-memory",
    title="harness self-test: memory limits",
    suite="selftest",
    implementations=("port",),
    draw=draw,
    case=case,
    run=run,
    exact_checks=(),
    timeout=120,
    family="python",
    payload_fields=(),
)
