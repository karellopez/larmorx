# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
"""FSL's applytopup runs, served by the oracle cache in use mode (new runs stored), each into a
directory of its own with its arguments, exit status and streams. Usage:
  at_case.py cases <outdir> [name-prefix]     every ``at/`` case of the replica suite
  at_case.py args <outdir> <source> -- <applytopup args...>   (@name: derived inputs;
                                                <source>: a topup source of TOPUP_SOURCES, or -)
"""

import sys
from pathlib import Path

from larmorx_validation.parity import fsl_topup_replica as T


def one(run, out: Path) -> None:
    out.mkdir(parents=True, exist_ok=True)
    args = T._prepare(run, out, T._source_node())
    p = T._oracle(run, args, out, dict(run.settings))
    (out / "ARGS.txt").write_text(" ".join(args) + "\n")
    (out / "STDOUT.txt").write_text(p.stdout)
    (out / "STDERR.txt").write_text(p.stderr)
    (out / "EXIT.txt").write_text(f"{p.code}\n")
    print(out.name, "exit", p.code)


def main():
    kind, out = sys.argv[1], Path(sys.argv[2])
    if kind == "cases":
        prefix = "at/" + (sys.argv[3] if len(sys.argv) > 3 else "")
        for name, (_, _, run) in T._all_cases().items():
            if name.startswith(prefix) and not run.refused:
                one(run, out / name[3:])
    else:
        source = sys.argv[3]
        i = sys.argv.index("--")
        run = T.Run(
            tuple(sys.argv[i + 1 :]),
            program="applytopup",
            source="" if source == "-" else source,
        )
        one(run, out)


main()
