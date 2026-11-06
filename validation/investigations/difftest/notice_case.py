# SPDX-License-Identifier: Apache-2.0
"""A finding file for a random case whose verdict was a *notice* (a D13 notice where the
correction changed nothing: such cases are counted, not shrunk, so no finding file is written),
so that ``difftest --promote`` can make it a permanent case.

Usage (``python -P``, with the repository's ``python/`` and ``validation/src`` on ``PYTHONPATH``)::

    notice_case.py <tool> <seed> <n> <out.json> "<defect>: no values changed"
    python -m larmorx_validation difftest --promote <out.json> --id <name> --status fixed ...

Used for fsl-prelude's FSL-261 notice (``ramps-notice-unchanged``, 2026-11-06)."""

from __future__ import annotations

import json
import sys
from pathlib import Path

from larmorx_validation.difftest import core, tools


def main() -> None:
    tool, seed, n, out, detail = (
        sys.argv[1],
        sys.argv[2],
        int(sys.argv[3]),
        sys.argv[4],
        sys.argv[5],
    )
    recipe = core.make_recipe(tools.get(tool), seed, n)
    doc = {
        "tool": tool,
        "implementation": tools.get(tool).implementations[0],
        "seed": seed,
        "n": n,
        "recipe": recipe,
        "signature": [f"notice: {detail.split(':')[0]}"],
        "detail": detail,
    }
    Path(out).write_text(json.dumps(doc, indent=1) + "\n", encoding="utf-8")
    print(" ".join(core.arguments(recipe)))


if __name__ == "__main__":
    main()
