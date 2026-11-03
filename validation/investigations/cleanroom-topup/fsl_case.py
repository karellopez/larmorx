# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
"""FSL's run of one case of the replica suite (or of given arguments), served by the oracle
cache in use mode, into a directory. Usage:
  fsl_case.py case <name> <outdir>
  fsl_case.py args <outdir> -- <topup args...>   (@name: derived inputs)
  fsl_case.py fmriprep <run> <outdir>
"""

import sys
from pathlib import Path

from larmorx_validation.parity import fsl_topup_replica as T


def main():
    kind = sys.argv[1]
    if kind == "case":
        name, out = sys.argv[2], Path(sys.argv[3])
        tier, desc, run = T._all_cases()[name]
        out.mkdir(parents=True, exist_ok=True)
        args = T._prepare(run, out, T._source_node())
        p = T._oracle(run, args, out, dict(run.settings))
    elif kind == "fmriprep":
        runname, out = sys.argv[2], Path(sys.argv[3])
        node = next(n for r, n in T.fmriprep_calls() if r == runname)
        out.mkdir(parents=True, exist_ok=True)
        assert T._prepare_fmriprep(node, out)
        args = T.fmriprep_args(node)
        p = T.run_oracle(args, cwd=out, settings={})
    else:
        out = Path(sys.argv[2])
        i = sys.argv.index("--")
        out.mkdir(parents=True, exist_ok=True)
        run = T.Run(tuple(sys.argv[i + 1 :]))
        args = T._prepare(run, out, T._source_node())
        p = T._oracle(run, args, out, {})
    (out / "ARGS.txt").write_text(" ".join(args) + "\n")
    (out / "STDOUT.txt").write_text(p.stdout)
    (out / "STDERR.txt").write_text(p.stderr)
    print("exit", p.code, "args", " ".join(args))


main()
