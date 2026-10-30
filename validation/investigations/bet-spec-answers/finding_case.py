# SPDX-License-Identifier: Apache-2.0
"""A random-testing finding's recipe (its shrunk one with ``--shrunk``) as a call to run by
hand: its program, arguments (the inputs built from their recipes, copied to ``<dir>``) and
``FSLOUTPUTTYPE``. ``python -P finding_case.py <finding.json> <dir> [--shrunk]``."""

from __future__ import annotations

import json
import shlex
import shutil
import sys
from pathlib import Path


def main(argv: list[str]) -> int:
    from larmorx_validation.difftest.tools import fsl_bet as tool
    from larmorx_validation.parity import fsl_bet as fb

    doc = json.loads(Path(argv[0]).read_text(encoding="utf-8"))
    d = Path(argv[1])
    recipe = doc["shrunk"] if "--shrunk" in argv[2:] and doc.get("shrunk") else doc["recipe"]
    d.mkdir(parents=True, exist_ok=True)
    s = tool.case(recipe, "original").payload
    paths = []
    for k, name in enumerate(s.inputs):
        p = fb.input_path(name)
        q = d / f"in{k}{''.join(p.suffixes)}"
        shutil.copyfile(p, q)
        paths.append(q)
    args = fb._arguments(s, paths)
    print(f"OT={s.output_type}", s.tool, " ".join(shlex.quote(a) for a in args))
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
