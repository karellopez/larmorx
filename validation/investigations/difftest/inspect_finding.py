# SPDX-License-Identifier: Apache-2.0
"""Look at a finding of the differential tests by hand: rebuild its shrunk inputs, run the
original program (twice: does it repeat itself?) and larmorx in both D13 modes, and print the
exit statuses, the last lines of their messages, the outputs' values and the header fields that
differ. ANTs' image programs only (the ``AntsCase`` tools); the parity runner itself is
``python -m larmorx_validation difftest --replay <finding>``.

    python -P validation/investigations/difftest/inspect_finding.py <finding.json> [<dir>]
"""

from __future__ import annotations

import contextlib
import io
import json
import os
import sys
import tempfile
from pathlib import Path

import nibabel as nib
import numpy as np
from larmorx_validation.difftest import tools
from larmorx_validation.parity import ants_programs as ap


def main() -> int:
    finding = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
    work = Path(sys.argv[2]) if len(sys.argv) > 2 else Path(tempfile.mkdtemp(prefix="lx-inspect-"))
    work.mkdir(parents=True, exist_ok=True)
    tool = tools.get(finding["tool"])
    case = tool.case(finding.get("shrunk") or finding["recipe"], finding["implementation"])
    c = case.payload
    print(f"{finding['tool']}: {c.program} {' '.join(c.args)}")
    runs = {}
    for name, runner in (
        ("ants1", "ants"),
        ("ants2", "ants"),
        ("lx-reproduce", "reproduce"),
        ("lx-fix", "fix"),
    ):
        out = work / name
        out.mkdir(exist_ok=True)
        args = [ap.resolve_arg(a, work, out) for a in c.args]
        outputs = [Path(ap.resolve_arg(o, work, out)) for o in c.outputs]
        if runner == "ants":
            env = {
                **os.environ,
                "ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS": "1",
                "LARMORX_ORACLE_CACHE_MODE": "off",
            }
            import subprocess

            p = subprocess.run(
                [str(ap.ants_bin() / c.program), *args],
                capture_output=True,
                text=True,
                env=env,
                check=False,
            )
            code, err = p.returncode, p.stderr + p.stdout
        else:
            from larmorx.cli import run

            e = io.StringIO()
            os.environ["LARMORX_UPSTREAM_DEFECTS"] = runner
            os.environ["ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS"] = "1"
            with contextlib.redirect_stderr(e), contextlib.redirect_stdout(io.StringIO()):
                code = run(["larmorx", "ants", c.program, *args])
            err = e.getvalue()
        runs[name] = outputs
        tail = " | ".join(err.strip().splitlines()[-3:])[:400]
        print(f"  {name}: exit {code}; {tail}")
        for o in outputs:
            if o.is_file():
                img = nib.load(o)
                v = np.asanyarray(img.dataobj.get_unscaled()).ravel()
                print(f"    {o.name}: {img.shape} {v.dtype} {v[:8]}")
    a, b = runs["ants1"], runs["lx-reproduce"]
    for x, y in zip(a, b, strict=False):
        if x.is_file() and y.is_file():
            diffs = ap.header_differences(ap.raw_header(x), ap.raw_header(y))
            print("  header differences (ANTs, larmorx):", "; ".join(diffs) or "none")
    return 0


if __name__ == "__main__":
    sys.exit(main())
