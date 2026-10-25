# SPDX-License-Identifier: Apache-2.0
"""A finding of the differential tests run by hand, for any ANTs program: its shrunk inputs
rebuilt, the ANTs program (one thread) and ``python -m larmorx ants <program>`` (both D13 modes)
on its arguments, each run's exit status and its full standard error, and the lines of standard
output that differ (addresses masked).

    python -P validation/investigations/difftest/run_pair.py <finding.json> <dir> <program>
"""

from __future__ import annotations

import difflib
import json
import os
import re
import subprocess
import sys
from pathlib import Path

from larmorx_validation.difftest import core, inputs
from larmorx_validation.parity import ants_programs


def main() -> int:
    finding = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
    work, program = Path(sys.argv[2]), sys.argv[3]
    work.mkdir(parents=True, exist_ok=True)
    recipe = finding.get("shrunk") or finding["recipe"]
    paths = {}
    for name, spec in recipe["inputs"].items():
        p = work / (name + inputs.suffix_of(spec))
        inputs.materialize(spec, p)
        paths[name] = str(p)
    words = core.substitute(core.arguments(recipe), lambda n: paths[n])
    env = {**os.environ, "ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS": "1", "ANTS_RANDOM_SEED": "42"}
    runs = {}
    for who in ("ANTs", "reproduce", "fix"):
        out = work / who
        out.mkdir(exist_ok=True)
        args = [w.replace("{out}", str(out)) for w in words]
        if who == "ANTs":
            cmd = [str(ants_programs.ants_bin() / program), *args]
            e = env
        else:
            cmd = [sys.executable, "-m", "larmorx", "ants", program, *args]
            e = {**env, "LARMORX_UPSTREAM_DEFECTS": who}
        p = subprocess.run(cmd, capture_output=True, text=True, env=e, check=False)
        runs[who] = p
        # Each run's messages, kept for a closer look.
        (out / "stdout.txt").write_text(p.stdout, encoding="utf-8")
        (out / "stderr.txt").write_text(p.stderr, encoding="utf-8")
        print(f"== {who}: exit {p.returncode}")
        print(p.stderr.replace(str(out), "{out}").rstrip() or "(no standard error)")

    def mask(s: str, who: str) -> list[str]:
        return re.sub(r"0x[0-9a-f]+", "0x", s.replace(str(work / who), "{out}")).splitlines()

    for who in ("reproduce", "fix"):
        diff = list(
            difflib.unified_diff(
                mask(runs["ANTs"].stdout, "ANTs"),
                mask(runs[who].stdout, who),
                "ANTs",
                who,
                n=0,
                lineterm="",
            )
        )
        print(
            f"== standard output, ANTs against larmorx {who}: "
            + ("identical" if not diff else f"{len(diff)} diff lines")
        )
        print("\n".join(diff[:40]))
    return 0


if __name__ == "__main__":
    sys.exit(main())
