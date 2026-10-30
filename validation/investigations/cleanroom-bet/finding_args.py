# SPDX-License-Identifier: Apache-2.0
"""A random-testing finding's call, to run by hand (``compare.py``): its program, arguments (the
inputs built from their recipes) and ``FSLOUTPUTTYPE``.
``python -m``-style: ``python finding_args.py <finding.json> [recipe|shrunk]``."""

from __future__ import annotations

import json
import shlex
import sys
from pathlib import Path


def main(argv: list[str]) -> int:
    from larmorx_validation.difftest.tools import fsl_bet as tool
    from larmorx_validation.parity import fsl_bet as fb

    d = json.loads(Path(argv[0]).read_text(encoding="utf-8"))
    which = argv[1] if len(argv) > 1 else "shrunk"
    recipe = d.get(which) or d["recipe"]
    case = tool.case(recipe, "original")
    s = case.payload
    paths = [fb.input_path(n) for n in s.inputs]
    args = fb._arguments(s, paths)
    print(f"OT={s.output_type}", s.tool, " ".join(shlex.quote(a) for a in args))
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
