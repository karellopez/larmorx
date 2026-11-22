# SPDX-License-Identifier: Apache-2.0
"""getfullpath's calls in the recorded runs: the official script against the port in both modes.

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (it runs the replica).

For every reference run (``oracles/freesurfer-8.2.0/runs/<run>/``), the calls its scripts' logs
show (``recon-all.log``): seg2cc's two (``$SUBJECTS_DIR/<subject>/mri/aseg.auto_noCCseg.mgz`` and
``norm.mgz``) and fs-synthmorph-reg's three (its ``--i``, ``--t`` and ``--o``), each run in the
subject's ``mri`` folder by the official script (through the oracle-output cache, never stored)
and by the port (``--port <larmorx-freesurfer>``) with ``reproduce`` and ``fix``; exit status and
both streams compared byte for byte.

    python -P validation/probes/fs-getfullpath/real.py --port <larmorx-freesurfer> [--json out.json]
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
sys.path.insert(0, str(REPO / "validation" / "src"))

from larmorx_validation import oracle_cache  # noqa: E402
from larmorx_validation.freesurfer_unit import FS_HOME  # noqa: E402
from larmorx_validation.oracle_cache.store import workspace  # noqa: E402

NEVER = "tcsh's patterns read the folder's whole listing, which the key does not hold"


def calls(log: Path) -> tuple[Path | None, list[str]]:
    text = log.read_text(encoding="utf-8", errors="replace")
    sd = re.search(r"^setenv SUBJECTS_DIR (\S+)$", text, re.M)
    out = []
    for m in re.finditer(r"^ seg2cc --s (\S+)", text, re.M):
        if sd:
            mri = Path(sd[1]) / m[1] / "mri"
            out += [str(mri / "aseg.auto_noCCseg.mgz"), str(mri / "norm.mgz")]
    for m in re.finditer(r"^fs-synthmorph-reg (.*)$", text, re.M):
        w = m[1].split()
        for opt in ("--i", "--t", "--o"):
            if opt in w:
                out.append(w[w.index(opt) + 1])
    return log.parent.parent / "mri", list(dict.fromkeys(out))


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--port", type=lambda s: Path(s).resolve(), required=True)
    ap.add_argument("--json", type=Path)
    a = ap.parse_args(argv)
    ws = workspace()
    exe = ws / FS_HOME / "bin" / "getfullpath"
    env = {"PATH": f"{exe.parent}:/usr/bin:/bin", "LC_ALL": "C", "LANG": "C"}
    rows = []
    for log in sorted(
        (ws / "oracles" / "freesurfer-8.2.0" / "runs").glob("*/subjects/*/scripts/recon-all.log")
    ):
        mri, args = calls(log)
        run = log.parents[3].name
        for arg in args:
            o = oracle_cache.run([exe.name, arg], env=env, cwd=mri, timeout=15,
                                 policy=oracle_cache.Policy(threads=1, family="freesurfer", never=NEVER),
                                 runner="fs-getfullpath-real")  # fmt: skip
            row = {"run": run, "arg": arg.replace(str(ws), "<workspace>")}
            for mode in ("reproduce", "fix"):
                p = subprocess.run([str(a.port), "--upstream-defects", mode, "freesurfer",
                                    "getfullpath", arg], env=env, cwd=mri, capture_output=True,
                                   timeout=15, check=False)  # fmt: skip
                row[mode] = (p.returncode, p.stdout, p.stderr) == (
                    o.returncode,
                    o.stdout_bytes,
                    o.stderr_bytes,
                )
            rows.append(row)
    summary = {
        "calls": len(rows),
        "runs": sorted({r["run"] for r in rows}),
        "reproduce_identical": sum(r["reproduce"] for r in rows),
        "fix_identical": sum(r["fix"] for r in rows),
        "rows": rows,
    }
    print(json.dumps({k: v for k, v in summary.items() if k != "rows"}, indent=1))
    if a.json:
        a.json.write_text(json.dumps(summary, indent=1) + "\n", encoding="utf-8")
    return 0 if summary["reproduce_identical"] == summary["fix_identical"] == len(rows) else 1


if __name__ == "__main__":
    raise SystemExit(main())
