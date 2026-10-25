# SPDX-License-Identifier: Apache-2.0
"""The random tests' mcflirt findings run again: each finding's shrunk recipe and its original
recipe, its inputs rebuilt, FSL 6.0.7's mcflirt and the replica (``reproduce``) on its
arguments, compared with the suite's own comparison (every file, both streams, the exit status)
and the replica's D13 fix pass. On 2026-10-25 all 61 findings (122 runs) were bit-identical with
the corrected ``atanf`` constant and the FSL-08 ``-meanvol`` notice.

    python -P difftest_replay.py <findings dir> <work dir> [n ...]

Needs the random-testing package (``larmorx_validation.difftest``) on ``PYTHONPATH`` and the
replica through ``LARMORX_NC_BIN``; findings are
``oracles/difftest/<date>/findings/fsl-mcflirt/<seed>-<n>-replica.json``.
"""

from __future__ import annotations

import json
import shutil
import sys
from pathlib import Path

from larmorx_validation.difftest import core, inputs
from larmorx_validation.parity import fsl_mcflirt_replica as fr
from larmorx_validation.parity.harness import CheckList


def main() -> int:
    findings, work = Path(sys.argv[1]), Path(sys.argv[2]).resolve()
    only = set(sys.argv[3:])
    bad, total = [], 0
    for f in sorted(findings.glob("*-replica.json")):
        n = f.name.split("-")[1].lstrip("0") or "0"
        if only and n not in only:
            continue
        finding = json.loads(f.read_text(encoding="utf-8"))
        for kind in ("shrunk", "recipe"):
            recipe = finding.get(kind)
            if not recipe:
                continue
            d = work / f"{n}-{kind}"
            if d.exists():
                shutil.rmtree(d)
            d.mkdir(parents=True)
            paths = {}
            for name, spec in recipe["inputs"].items():
                p = d / (name + inputs.suffix_of(spec))
                inputs.materialize(spec, p)
                paths[name] = str(p)
            line = " ".join(
                core.substitute(core.arguments(recipe), lambda k, paths=paths: paths[k])
            )
            runs = {}
            for who in ("mcflirt", "replica"):
                w = d / who
                w.mkdir()
                args = line.replace("{run}", str(w)).split()
                if who == "mcflirt":
                    runs[who] = fr.run_oracle(args, cwd=w, settings={})
                else:
                    runs[who] = fr.run_replica(
                        args, cwd=w, settings={}, mode="reproduce", threads=1
                    )
            checks = CheckList()
            fr.compare(
                checks, runs["mcflirt"], runs["replica"], d / "mcflirt", d / "replica", set()
            )
            if runs["replica"].code == 0:

                def args_for(t: Path, line: str = line) -> list[str]:
                    return line.replace("{run}", str(t)).split()

                fr.fix_pass(checks, args_for, {}, d / "replica", runs["replica"], set())
            failed = [c for c in checks.checks if not c.passed]
            total += 1
            codes = f"{runs['mcflirt'].code}/{runs['replica'].code}"
            print(f"{n} {kind}: {'DIFFER' if failed else 'SAME'} (exit {codes})", flush=True)
            for c in failed:
                print("    ", c.name, "|", (c.detail or "")[:300], flush=True)
            if failed:
                bad.append(f"{n}-{kind}")
    print(f"{total} runs compared; differing: {len(bad)} {bad}")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
