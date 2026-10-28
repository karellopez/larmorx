# SPDX-License-Identifier: Apache-2.0
"""The allocations the parity cases that expect a refusal ask for: each original's run traced
(``strace -f -e trace=mmap,mremap,brk``) under the address-space limit, and the largest request
the kernel refused (``ENOMEM``) reported.

Usage (from the checkout, the environment of ``scripts/orchestration/env.sh``, in a memory box)::

    python -P refused_sizes.py <out>

The cases: ``difftest/allocation-refused`` of ResampleImage, ResampleImageBySpacing and
N4BiasFieldCorrection, and ImageMath's ``binary-morphology/MD-radius-negative``. Writes
``<out>/refused.json`` and prints one line per case: the smallest of them bounds the limit from
above (``larmorx_validation.address_space``).
"""

from __future__ import annotations

import json
import re
import shutil
import sys
from pathlib import Path

from larmorx_validation import address_space
from larmorx_validation.oracle_cache import core

CASES = (
    ("ants-resample-image", "difftest/allocation-refused"),
    ("ants-resample-image-by-spacing", "difftest/allocation-refused"),
    ("ants-n4", "difftest/allocation-refused"),
    ("ants-image-math", "binary-morphology/MD-radius-negative"),
)
_REFUSED = re.compile(r"\b(mmap|mremap)\((?:NULL|0x[0-9a-f]+), (\d+),.*= -1 ENOMEM")


def main(argv: list[str] | None = None) -> int:
    from larmorx_validation.__main__ import main as lxv

    out = Path((argv or sys.argv[1:])[0])
    out.mkdir(parents=True, exist_ok=True)
    strace = shutil.which("strace")
    if strace is None:
        print("strace is not installed")
        return 1
    real = core._execute
    traces: list[Path] = []

    def traced(args, env, cwd, timeout):
        log = out / f"trace-{len(traces)}.txt"
        traces.append(log)
        return real([strace, "-f", "-e", "trace=mmap,mremap,brk", "-o", str(log), *args],
                    env, cwd, timeout)  # fmt: skip

    core._execute = traced
    rows = []
    for suite, case in CASES:
        first = len(traces)
        lxv(["parity", suite, "--tier", "smoke", "--case", case, "--oracle-cache", "off",
             "--out", str(out / "records")])  # fmt: skip
        sizes = []
        for log in traces[first:]:
            for line in log.read_text(errors="replace").splitlines():
                if m := _REFUSED.search(line):
                    sizes.append(int(m.group(2)))
        largest = max(sizes, default=0)
        rows.append({"suite": suite, "case": case, "refused_bytes": largest})
        print(f"{suite:32s} {case:40s} refused {largest:>22d} bytes ({largest / 2**30:,.1f} GiB)")
    (out / "refused.json").write_text(
        json.dumps({"limit": address_space.limit(), "cases": rows}, indent=1) + "\n",
        encoding="utf-8",
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
