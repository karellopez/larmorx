# SPDX-License-Identifier: Apache-2.0
"""FSL's fslstats on cases of the replica suite (``fsl-stats-replica``) or on ad-hoc command
lines: black-box runs of FSL 6.0.7.23's program through the oracle-output cache (served when
they ran before), printing the exit status, standard output and standard error.

Run (in a memory box): ``python -P show_fslstats.py <case or 'args:...'> ...``

- ``<case>``: a case of the replica suite's synthetic cases (``opt/-h``); its FSL-test cases
  (``fsltest/``: FSL's own test script and images, in FSL's source tree) are never listed: the
  clean-room implementer does not open them;
- ``args:<arg> <arg> ...``: fslstats' arguments, ``@name`` naming an input of the replica suite
  (copied into the run directory) or ``@orig:<name>`` one of the clean-room suite's own
  (``fsl_stats.own_inputs``).
"""

from __future__ import annotations

import sys
import tempfile
from pathlib import Path

from larmorx_validation.parity import fsl_stats as fs
from larmorx_validation.parity import fsl_stats_replica as rep


def main(argv: list[str]) -> int:
    cases = fs.replica_cases()
    for item in argv:
        with tempfile.TemporaryDirectory() as t:
            d = Path(t)
            if item.startswith("args:"):
                run = rep._r(*item[len("args:") :].split())
                label = item
            else:
                run = cases[item][2]
                label = item
            args = fs._prepare(run, d)
            p = rep.run_oracle(args, cwd=d, settings=dict(run.settings))
            print(f"=== {label}: fslstats {' '.join(args)}\nexit {p.code}")
            print(f"--stdout\n{p.stdout!r}\n--stderr\n{p.stderr!r}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
