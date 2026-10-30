# SPDX-License-Identifier: Apache-2.0
"""A random-testing night's case (seed and number) as a call to run by hand (``compare.py``):
its program, arguments (the inputs built from their recipes, copied to ``<dir>``) and
``FSLOUTPUTTYPE``. ``python night_case.py <seed> <n> <dir>``."""

from __future__ import annotations

import shlex
import shutil
import sys
from pathlib import Path


def main(argv: list[str]) -> int:
    from larmorx_validation.difftest import core
    from larmorx_validation.difftest.tools import fsl_bet as tool
    from larmorx_validation.parity import fsl_bet as fb

    seed, n, d = argv[0], int(argv[1]), Path(argv[2])
    d.mkdir(parents=True, exist_ok=True)
    recipe = core.make_recipe(tool.TOOL, seed, n)
    s = tool.case(recipe, "original").payload
    paths = []
    for k, name in enumerate(s.inputs):
        p = fb.input_path(name)
        q = d / f"in{k}_{n}{''.join(p.suffixes)}"
        shutil.copyfile(p, q)
        paths.append(q)
    args = fb._arguments(s, paths)
    print(f"OT={s.output_type}", s.tool, " ".join(shlex.quote(a) for a in args))
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
