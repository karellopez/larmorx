# SPDX-License-Identifier: Apache-2.0
"""The program nodes of fMRIPrep oracle runs that the pipeline-stage catalogue does not classify
(neither a stage larmorx compares nor a roadmap entry), the check of
``tests/parity/test_pipeline_stages.py::test_every_program_node_is_classified``, run over every
finished run with a summary: each unclassified node's normalised path and program, grouped.

    PYTHONPATH=<checkout>/python:<checkout>/validation/src \\
        python -P unclassified.py <workspace>/oracles/fmriprep-25.2.6 [run ...]
"""

from __future__ import annotations

import collections
import sys
from pathlib import Path

from larmorx_validation.stages import catalogue
from larmorx_validation.stages.oracle import OracleRun


def finished(run: Path) -> bool:
    info = run / "RUN_INFO.txt"
    return info.is_file() and "finished:" in info.read_text(encoding="utf-8")


def main() -> int:
    root = Path(sys.argv[1])
    names = sys.argv[2:] or sorted(p.name for p in root.iterdir() if (p / "work").is_dir())
    total = 0
    for name in names:
        run_dir = root / name
        if not finished(run_dir):
            print(f"{name}: not finished, skipped")
            continue
        run = OracleRun(run_dir)
        groups: dict[tuple[str, str], list[str]] = collections.defaultdict(list)
        programs = collections.Counter()
        for n in run.all_nodes:
            if not n.program:
                continue
            programs[n.program] += 1
            if catalogue.stage_of(n) is None and catalogue.missing_of(n) is None:
                norm, _ = catalogue.normalise(n.path)
                groups[(n.program, norm)].append(n.path)
        n_un = sum(len(v) for v in groups.values())
        total += n_un
        print(f"{name}: {sum(programs.values())} program nodes, {n_un} unclassified")
        for (program, norm), paths in sorted(groups.items()):
            print(f"  {program:24s} {norm}  ({len(paths)}; e.g. {paths[0]})")
    print(f"unclassified in all: {total}")
    return 1 if total else 0


if __name__ == "__main__":
    sys.exit(main())
