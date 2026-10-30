# SPDX-License-Identifier: Apache-2.0
"""Case by case, the runs of ``mcflirt_fsl20_21.py`` side by side: exit status, whether the
matrices and the files written are the first run's (mcflirt's), the last lines of standard error.

    python -P validation/investigations/specs-memory/compare_runs.py <fsl-run> <other-run>...
"""

from __future__ import annotations

import json
import sys
from pathlib import Path


def main() -> None:
    dirs = [Path(a) for a in sys.argv[1:]]
    runs = {d.name: json.loads((d / "results.json").read_text(encoding="utf-8")) for d in dirs}
    first = runs[dirs[0].name]
    for case, f in first.items():
        print(f"== {case}  {' '.join(f['args'])}")
        for name, r in runs.items():
            c = r.get(case)
            if not c:
                continue
            same = ""
            if c is not f:
                same = "mats=" + ("same" if c["mats"] == f["mats"] else "DIFF")
                same += " par=" + ("same" if c.get("par") == f.get("par") else "DIFF")
                same += " files=" + ("same" if c["files"] == f["files"] else "DIFF")
            err = [line for line in c["stderr"].splitlines() if line.strip()][-2:]
            tail = " / ".join(e[:170] for e in err)
            print(f"  {name:28s} exit {c['returncode']:4d} {same}  | {tail}")


if __name__ == "__main__":
    main()
